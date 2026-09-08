use anyhow::Result;
use anyrender::ImageRenderer;
use anyrender_skia::SkiaImageRenderer;
use base64::Engine;
use blitz_dom::DocumentConfig;
pub use blitz_dom::Point;
use blitz_html::HtmlDocument;
use blitz_traits::{
    net::{NetHandler, NetProvider, Request},
    shell::{ColorScheme, Viewport},
};
use std::{path::PathBuf, sync::Arc};

/// No networking, executable content, or ambient file access. Canonical paths prevent symlink escapes.
struct Resources {
    root: Option<PathBuf>,
    assets: PathBuf,
}
impl NetProvider for Resources {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let bytes = match request.url.scheme() {
            "data" => request
                .url
                .as_str()
                .split_once(',')
                .and_then(|(header, data)| {
                    if header.ends_with(";base64") && data.len() <= 32 * 1024 * 1024 {
                        base64::engine::general_purpose::STANDARD.decode(data).ok()
                    } else {
                        None
                    }
                }),
            "file" => request
                .url
                .to_file_path()
                .ok()
                .and_then(|p| p.canonicalize().ok())
                .filter(|p| {
                    p.starts_with(&self.assets)
                        || self.root.as_ref().is_some_and(|root| p.starts_with(root))
                })
                .and_then(|p| {
                    if std::fs::metadata(&p).ok()?.len() > 32 * 1024 * 1024 {
                        return None;
                    }
                    std::fs::read(p).ok()
                }),
            _ => None,
        };
        // Always complete rejected requests as well, so the DOM cannot remain render-blocked.
        handler.bytes(request.url.to_string(), bytes.unwrap_or_default().into());
    }
}

pub struct NativeDocument {
    pub document: HtmlDocument,
    painter: SkiaImageRenderer,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    inline_node: Option<blitz_traits::node_id::NodeId>,
    inline_resized: bool,
}

#[derive(Clone, Debug)]
pub struct BlockHit {
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl NativeDocument {
    pub fn new(
        html: &str,
        root: Option<PathBuf>,
        assets: PathBuf,
        width: u32,
        height: u32,
        scale: f32,
        dark: bool,
    ) -> Result<Self> {
        let root = root.and_then(|p| p.canonicalize().ok());
        let assets = assets.canonicalize()?;
        let base = root.as_ref().unwrap_or(&assets);
        let config = DocumentConfig {
            viewport: Some(Viewport::new(
                width,
                height,
                scale,
                if dark {
                    ColorScheme::Dark
                } else {
                    ColorScheme::Light
                },
            )),
            base_url: Some(
                url::Url::from_directory_path(base)
                    .map_err(|_| anyhow::anyhow!("无效资源路径"))?
                    .to_string(),
            ),
            net_provider: Some(Arc::new(Resources { root, assets })),
            ..Default::default()
        };
        let mut document = HtmlDocument::from_html(html, config);
        // Inline/local resources are delivered synchronously; a second pass incorporates discovered fonts/images.
        for _ in 0..3 {
            document.resolve(0.0);
        }
        Ok(Self {
            document,
            painter: SkiaImageRenderer::new(width, height),
            width,
            height,
            scale,
            inline_node: None,
            inline_resized: false,
        })
    }
    pub fn paint(&mut self, offset: u32) -> Vec<u8> {
        self.document.set_viewport_scroll(Point {
            x: 0.,
            y: offset as f64 / self.scale as f64,
        });
        self.painter.reset();
        let mut pixels = Vec::new();
        self.painter.render_to_vec(
            |scene| {
                blitz_paint::paint_scene(
                    scene,
                    &mut self.document,
                    self.scale as f64,
                    self.width,
                    self.height,
                    0,
                    0,
                );
            },
            &mut pixels,
        );
        pixels
    }
    /// Reserve space in the native document flow for the native IME-enabled source input.
    /// Only the selected node changes; all following blocks retain normal CSS layout.
    pub fn text_at_point(&self, x: f32, y: f32) -> Option<(String, usize, f32)> {
        let (id, byte) = self
            .document
            .find_text_position(x, y + self.document.viewport_scroll().y as f32)?;
        let layout = self
            .document
            .get_node(id)?
            .element_data()?
            .inline_layout_data
            .as_ref()?;
        Some((
            layout.text.clone(),
            byte,
            self.document
                .get_node(id)?
                .primary_styles()?
                .clone_font_size()
                .used_size()
                .px(),
        ))
    }
    pub fn begin_inline(&mut self, x: f32, y: f32) -> Option<BlockHit> {
        let target = self.block_at(x, y)?;
        self.begin_inline_source(target.start)
    }
    /// Find a semantic Markdown block from its stable source offset.
    pub fn block_from_source(&self, start: usize) -> Option<BlockHit> {
        let id = self
            .document
            .query_selector_all("[data-source-start]")
            .ok()?
            .into_iter()
            .rev()
            .find(|&id| {
                let node = self.document.get_node(id).unwrap();
                node.attr("data-source-start".into())
                    .and_then(|s| s.parse::<usize>().ok())
                    == Some(start)
            })?;
        let node = self.document.get_node(id)?;
        let end = node.attr("data-source-end".into())?.parse::<usize>().ok()?;
        let rect = self.document.get_client_bounding_rect(id)?;
        Some(BlockHit {
            start,
            end,
            x: rect.x as f32,
            y: rect.y as f32,
            width: rect.width as f32,
            height: rect.height as f32,
        })
    }
    pub fn inline_source_range(&self) -> Option<(usize, usize)> {
        let node = self.document.get_node(self.inline_node?)?;
        Some((
            node.attr("data-source-start".into())?.parse().ok()?,
            node.attr("data-source-end".into())?.parse().ok()?,
        ))
    }
    fn finish_inline(&mut self) {
        let Some(id) = self.inline_node.take() else {
            return;
        };
        self.document
            .set_style_property(id, "visibility", "visible");
        if self.inline_resized {
            self.document.set_style_property(id, "overflow", "visible");
            self.document.set_style_property(id, "height", "auto");
            self.inline_resized = false;
        }
        self.document.resolve(0.);
    }
    pub fn begin_inline_source(&mut self, start: usize) -> Option<BlockHit> {
        // Measure in normal flow so the new input lands exactly where the rendered block was.
        self.finish_inline();
        let hit = self.block_from_source(start)?;
        let id = self
            .document
            .query_selector_all("[data-source-start]")
            .ok()?
            .into_iter()
            .rev()
            .find(|&id| {
                let node = self.document.get_node(id).unwrap();
                node.attr("data-source-start".into())
                    .and_then(|s| s.parse::<usize>().ok())
                    == Some(hit.start)
                    && node
                        .attr("data-source-end".into())
                        .and_then(|s| s.parse::<usize>().ok())
                        == Some(hit.end)
            })?;
        self.inline_node = Some(id);
        // Keep descendants alive: Blitz inline layout damage caches can still reference them.
        // CSS visibility removes painting without invalidating those node IDs.
        self.document.set_style_property(id, "visibility", "hidden");
        // Do not touch geometry here. The fixed-height overlay scrolls internally, while the
        // hidden rendered node retains its exact margins, padding, and downstream flow.
        self.document.resolve(0.);
        Some(hit)
    }
    pub fn reserve_inline(&mut self, height: f32) {
        if let Some(id) = self.inline_node {
            self.inline_resized = true;
            self.document.set_style_property(id, "overflow", "hidden");
            self.document
                .set_style_property(id, "height", &format!("{}px", height.max(24.)));
            self.document.set_style_property(id, "min-height", "0");
            self.document.set_style_property(id, "padding", "0");
            self.document.set_style_property(id, "border", "0");
            self.document.resolve(0.);
        }
    }
    pub fn inline_rect(&self) -> Option<(f32, f32, f32)> {
        let rect = self.document.get_client_bounding_rect(self.inline_node?)?;
        Some((rect.x as f32, rect.y as f32, rect.width as f32))
    }
    pub fn select_at(&mut self, phase: i32, x: f32, y: f32) {
        let y = y + self.document.viewport_scroll().y as f32;
        if phase == 0 {
            self.document.clear_text_selection();
            if let Some((node, offset)) = self.document.find_text_position(x, y) {
                self.document.set_text_selection(node, offset, node, offset);
            }
        } else {
            self.document.extend_text_selection_to_point(x, y);
        }
    }
    pub fn selected_text(&self) -> Option<String> {
        // Preserve paragraph boundaries; Blitz's default accessor joins every inline root with a space.
        let mut text = String::new();
        let mut previous_y: Option<f64> = None;
        for (id, start, end) in self.document.get_text_selection_ranges() {
            let node = self.document.get_node(id)?;
            let layout = node.element_data()?.inline_layout_data.as_ref()?;
            let part = layout.text.get(start..end)?;
            if part.is_empty() {
                continue;
            }
            let y = self.document.get_client_bounding_rect(id).map(|r| r.y);
            if !text.is_empty() {
                text.push_str(
                    if previous_y.zip(y).is_some_and(|(a, b)| (a - b).abs() < 0.5) {
                        "\t"
                    } else {
                        "\n\n"
                    },
                );
            }
            text.push_str(part);
            previous_y = y;
        }
        (!text.is_empty()).then_some(text)
    }
    pub fn content_height(&self) -> f32 {
        self.document
            .root_element()
            .final_layout()
            .size
            .height
            .max(self.height as f32 / self.scale)
    }
    pub fn block_at(&self, x: f32, y: f32) -> Option<BlockHit> {
        let mut id = self
            .document
            .hit(x, y + self.document.viewport_scroll().y as f32)?
            .node_id;
        loop {
            let node = self.document.get_node(id)?;
            if let (Some(start), Some(end)) = (
                node.attr("data-source-start".into()),
                node.attr("data-source-end".into()),
            ) {
                let rect = self.document.get_client_bounding_rect(id)?;
                return Some(BlockHit {
                    start: start.parse().ok()?,
                    end: end.parse().ok()?,
                    x: rect.x as f32,
                    y: rect.y as f32,
                    width: rect.width as f32,
                    height: rect.height as f32,
                });
            }
            id = node.parent?;
        }
    }
}

pub fn katex_css(assets: &std::path::Path) -> Result<String> {
    let base =
        url::Url::from_directory_path(assets).map_err(|_| anyhow::anyhow!("字体路径无效"))?;
    Ok(include_str!("../../../vendor/katex/katex.min.css")
        .replace("url(fonts/", &format!("url({base}fonts/")))
}

pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(rgba)?;
    }
    Ok(bytes)
}

pub fn write_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

pub fn export_pdf(
    html: &str,
    path: &std::path::Path,
    root: Option<PathBuf>,
    assets: PathBuf,
) -> Result<()> {
    use anyrender_skia::{SkiaSceneCache, SkiaScenePainter};
    let mut native = NativeDocument::new(html, root, assets, 596, 788, 1., false)?;
    let height = native.content_height();
    let blocks = native
        .document
        .query_selector_all(".hui-block")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| native.document.get_client_bounding_rect(id))
        .map(|r| (r.y as f32, (r.y + r.height) as f32))
        .collect::<Vec<_>>();
    let mut bytes = Vec::new();
    let mut pdf = skia_safe::pdf::new_document(&mut bytes, None);
    let mut cache = SkiaSceneCache::new();
    let mut offset = 0f32;
    while offset < height {
        let mut end = (offset + 788.).min(height);
        // Keep normal blocks intact. Oversized blocks require finer line-level pagination.
        for &(top, bottom) in &blocks {
            if top > offset + 1. && top < end && bottom > end {
                end = top;
            }
        }
        native.document.set_viewport_scroll(Point {
            x: 0.,
            y: offset as f64,
        });
        let mut page = pdf.begin_page((595.5, 842.), None);
        let canvas = page.canvas();
        canvas.clip_rect(
            skia_safe::Rect::new(0., 24., 595.5, 24. + (end - offset)),
            None,
            false,
        );
        let mut painter = SkiaScenePainter::new(canvas, &mut cache);
        // SkiaScenePainter sets absolute matrices: scale must be supplied to the scene generator.
        blitz_paint::paint_scene(
            &mut painter,
            &mut native.document,
            1.0,
            596,
            (end - offset).ceil() as u32,
            0,
            24,
        );
        pdf = page.end_page();
        offset = end;
    }
    pdf.close();
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_source_reserves_document_flow_without_covering_following_blocks() {
        let html = "<html><style>body{margin:20px} p{margin:0 0 18px}</style><body><div data-source-start='0' data-source-end='8'><p data-source-start='0' data-source-end='8'>first <strong>bold</strong> <code>code</code></p></div><p id='after' data-source-start='9' data-source-end='15'>second</p></body></html>";
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        let mut native = NativeDocument::new(html, None, assets, 600, 400, 1., false).unwrap();
        let ids = native.document.query_selector_all("p").unwrap();
        let first = native.document.get_client_bounding_rect(ids[0]).unwrap();
        let before = native.document.get_client_bounding_rect(ids[1]).unwrap().y;
        native
            .begin_inline(first.x as f32 + 2., first.y as f32 + 2.)
            .unwrap();
        native.reserve_inline(160.);
        let after = native.document.get_client_bounding_rect(ids[1]).unwrap().y;
        assert!(
            after > before + 100.,
            "next block must move below source input: {before} -> {after}"
        );
        let second = native.document.get_client_bounding_rect(ids[1]).unwrap();
        let switched = native
            .begin_inline(second.x as f32 + 2., second.y as f32 + 2.)
            .unwrap();
        assert_eq!(switched.start, 9);
        assert!(
            (switched.y as f64 - before).abs() < 1.,
            "switching must restore the previous block before measuring the next"
        );
        let (_, y, _) = native.inline_rect().unwrap();
        native.paint(30);
        assert!((native.inline_rect().unwrap().1 - (y - 30.)).abs() < 1.);
        native.reserve_inline(24.);
        assert!(native.document.get_client_bounding_rect(ids[1]).unwrap().y < after - 100.);
    }
    #[test]
    fn aggregate_block_keeps_collapsed_margin_slot() {
        let html = "<html><style>body{margin:20px} h1{margin:0 0 24px} p{margin:0}</style><body><div class='hui-block' data-source-start='0' data-source-end='8'><h1>Heading</h1></div><div class='hui-block' data-source-start='9' data-source-end='15'><p>following</p></div></body></html>";
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        let mut native = NativeDocument::new(html, None, assets, 600, 400, 1., false).unwrap();
        let blocks = native.document.query_selector_all(".hui-block").unwrap();
        let first = native.document.get_client_bounding_rect(blocks[0]).unwrap();
        let before = native
            .document
            .get_client_bounding_rect(blocks[1])
            .unwrap()
            .y;
        let hit = native
            .begin_inline(first.x as f32 + 2., first.y as f32 + 2.)
            .unwrap();
        let after = native
            .document
            .get_client_bounding_rect(blocks[1])
            .unwrap()
            .y;
        assert!(hit.height > 0.);
        assert!(
            (after - before).abs() < 1.,
            "collapsed margin slot must remain stable: {before} -> {after}"
        );
    }
    #[test]
    fn welcome_complex_blocks_keep_all_following_positions() {
        let source = include_str!("../../hui-app/assets/welcome.md");
        let mut engine = crate::markdown::MarkdownEngine::new().unwrap();
        let rendered = engine
            .render(
                source,
                false,
                &Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
            .unwrap();
        let html = crate::markdown::wrap_html(&rendered.html, 0, 17., 780., "").replace(
            "</head>",
            "<style>body{line-height:1.65}p{margin-bottom:18px}</style></head>",
        );
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        let probe = NativeDocument::new(&html, None, assets.clone(), 900, 1200, 1., false).unwrap();
        let starts = probe
            .document
            .query_selector_all(".hui-block")
            .unwrap()
            .into_iter()
            .filter_map(|id| {
                probe
                    .document
                    .get_node(id)?
                    .attr("data-source-start".into())?
                    .parse::<usize>()
                    .ok()
            })
            .collect::<Vec<_>>();
        for start in starts {
            let mut native =
                NativeDocument::new(&html, None, assets.clone(), 900, 1200, 1., false).unwrap();
            let ids = native.document.query_selector_all(".hui-block").unwrap();
            let before = ids
                .iter()
                .map(|&id| native.document.get_client_bounding_rect(id).unwrap().y)
                .collect::<Vec<_>>();
            native.begin_inline_source(start).unwrap();
            for (id, expected) in ids.into_iter().zip(before) {
                let block_start = native
                    .document
                    .get_node(id)
                    .unwrap()
                    .attr("data-source-start".into())
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if block_start <= start {
                    continue;
                }
                let actual = native.document.get_client_bounding_rect(id).unwrap().y;
                assert!(
                    (actual - expected).abs() <= 1.,
                    "opening source block {start} moved a later block: {expected} -> {actual}"
                );
            }
        }
    }
    #[test]
    fn heading_levels_use_their_own_geometry() {
        let source = "# 一级标题\n\n## 二级标题\n\n### 三级标题\n\n后文\n";
        let mut engine = crate::markdown::MarkdownEngine::new().unwrap();
        let rendered = engine
            .render(
                source,
                false,
                &Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
            .unwrap();
        let html = crate::markdown::wrap_html(&rendered.html, 0, 17., 780., "");
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        for level in 1..=3 {
            let mut native =
                NativeDocument::new(&html, None, assets.clone(), 900, 700, 1., false).unwrap();
            let heading = native
                .document
                .query_selector_all(&format!("h{level}"))
                .unwrap()[0];
            let node = native.document.get_node(heading).unwrap();
            let start = node
                .attr("data-source-start".into())
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let expected = native.document.get_client_bounding_rect(heading).unwrap();
            let hit = native.begin_inline_source(start).unwrap();
            assert!((hit.y as f64 - expected.y).abs() < 0.5);
            assert!((hit.height as f64 - expected.height).abs() < 0.5);
        }
    }
    #[test]
    fn native_image_decodes_png_and_keeps_markdown_source_mapping() {
        let rgba = [220, 40, 60, 255].repeat(48 * 32);
        let png = encode_png(48, 32, &rgba).unwrap();
        let uri = base64::engine::general_purpose::STANDARD.encode(png);
        let source = format!("![测试图片](data:image/png;base64,{uri})\n");
        let mut engine = crate::markdown::MarkdownEngine::new().unwrap();
        let rendered = engine
            .render(
                &source,
                false,
                &Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
            .unwrap();
        let html = crate::markdown::wrap_html(&rendered.html, 0, 17., 780., "");
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        let mut native = NativeDocument::new(&html, None, assets, 600, 400, 1., false).unwrap();
        let id = native.document.query_selector_all("img").unwrap()[0];
        let rect = native.document.get_client_bounding_rect(id).unwrap();
        assert_eq!((rect.width as u32, rect.height as u32), (48, 32));
        let hit = native
            .block_at(rect.x as f32 + 2., rect.y as f32 + 2.)
            .unwrap();
        assert!(source[hit.start..hit.end].contains("![测试图片]"));
        let pixels = native.paint(0);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] > 200 && p[1] < 60 && p[2] < 80)
                .count()
                > 1000
        );
    }
    #[test]
    fn reading_selection_crosses_paragraphs_and_paints_highlight() {
        let html = "<html><style>body {margin:20px;font:17px sans-serif;} p {margin:20px 0;}</style><body><p>Alpha 中文 first paragraph</p><p>Beta second paragraph</p></body></html>";
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        let mut native = NativeDocument::new(html, None, assets, 600, 400, 1., false).unwrap();
        let ids = native.document.query_selector_all("p").unwrap();
        let a = native.document.get_client_bounding_rect(ids[0]).unwrap();
        let b = native.document.get_client_bounding_rect(ids[1]).unwrap();
        let before = native.paint(0);
        native.select_at(0, a.x as f32 + 1., a.y as f32 + 8.);
        native.select_at(1, (b.x + b.width - 2.) as f32, b.y as f32 + 8.);
        let text = native.selected_text().unwrap();
        assert!(text.contains("Alpha 中文 first paragraph"), "{text}");
        assert!(
            text.contains("paragraph\n\nBeta second paragraph"),
            "{text}"
        );
        assert_ne!(before, native.paint(0));
    }
    #[test]
    fn block_hit_preserves_utf8_source_ranges_and_scroll_coordinates() {
        let source = "# 中文标题\n\n第一段 👩‍💻。\n\n第二段。\n";
        let mut engine = crate::markdown::MarkdownEngine::new().unwrap();
        let rendered = engine
            .render(
                source,
                false,
                &Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
            .unwrap();
        let html = crate::markdown::wrap_html(&rendered.html, 0, 17., 780., "");
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex");
        let mut native = NativeDocument::new(&html, None, assets, 600, 500, 1., false).unwrap();
        let ids = native.document.query_selector_all(".hui-block").unwrap();
        assert_eq!(ids.len(), 3);
        for id in ids {
            let node = native.document.get_node(id).unwrap();
            let start: usize = node
                .attr("data-source-start".into())
                .unwrap()
                .parse()
                .unwrap();
            let end: usize = node
                .attr("data-source-end".into())
                .unwrap()
                .parse()
                .unwrap();
            assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
            assert!(!source[start..end].trim().is_empty());
            let rect = native.document.get_client_bounding_rect(id).unwrap();
            let hit = native
                .block_at(rect.x as f32 + 3., rect.y as f32 + 3.)
                .unwrap();
            assert_eq!((hit.start, hit.end), (start, end));
        }
        let first = native.paint(0);
        let scrolled = native.paint(40);
        assert_eq!(first.len(), 600 * 500 * 4);
        assert_ne!(first, scrolled);
    }
}
