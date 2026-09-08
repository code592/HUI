use crate::i18n::{message, tr};
use crate::{AppWindow, HeadingRow};
use hui_render::{
    markdown::{MarkdownEngine, wrap_html},
    native::{NativeDocument, katex_css, write_png},
};
use ropey::Rope;
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
};

pub type RenderCacheKey = (u64, u64, u32, u32, u32, i32, u32, u32, u32, u32, usize);

struct CachedDocument {
    document: NativeDocument,
    source: Rope,
    headings: Vec<(String, i32, i32)>,
    diagnostics: Vec<String>,
}

enum Request {
    Document {
        generation: u64,
        cache_key: RenderCacheKey,
        source: Rope,
        root: Option<PathBuf>,
        width: u32,
        height: u32,
        scale: f32,
        theme: i32,
        font: f32,
        line_width: f32,
        line_height: f32,
        paragraph_spacing: f32,
        offset: f32,
    },
    Scroll(f32),
    Heading(usize),
    PrepareInlineSwitch {
        generation: u64,
        x: f32,
        y: f32,
    },
    FormulaPreview {
        generation: u64,
        source: String,
        width: f32,
        height: f32,
        scale: f32,
        theme: i32,
        font: f32,
    },
    Hit(f32, f32),
    Select(i32, f32, f32),
    Copy,
}
struct InlineSwitch {
    target_start: usize,
    local_x: f32,
    local_y: f32,
    active_end: usize,
    old_source_len: usize,
    generation: u64,
}
pub struct Renderer {
    sender: mpsc::Sender<Request>,
    generation: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
}

pub fn assets() -> PathBuf {
    if let Ok(p) = std::env::var("HUI_ASSETS") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        for path in [
            exe.parent().unwrap().join("../Resources/katex"),
            exe.parent().unwrap().join("assets/katex"),
        ] {
            if path.join("katex.min.css").exists() {
                return path;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex")
}

impl Renderer {
    pub fn new(ui: slint::Weak<AppWindow>) -> Self {
        let trace = std::env::var_os("HUI_TRACE").is_some();
        let (sender, receiver) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let serial = generation.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = cancel.clone();
        std::thread::Builder::new()
            .name("hui-layout".into())
            .spawn(move || {
                let mut engine = match MarkdownEngine::new() {
                    Ok(e) => e,
                    Err(e) => {
                        let _ = ui.upgrade_in_event_loop(move |ui| {
                            ui.set_notice(message("排版引擎初始化失败：{error}", &[("error", tr(&e.to_string()))]).into())
                        });
                        return;
                    }
                };
                engine.set_translator(tr);
                let mut document: Option<NativeDocument> = None;
                let mut current_generation = 0;
                let mut current_cache_key: Option<RenderCacheKey> = None;
                let mut current_source = Rope::new();
                let mut current_headings = Vec::new();
                let mut current_diagnostics = Vec::new();
                let mut cache: VecDeque<(RenderCacheKey, CachedDocument)> = VecDeque::new();
                let mut offset = 0.;
                let mut pending = None;
                let mut inline_switch: Option<InlineSwitch> = None;
                while let Some(mut request) = pending.take().or_else(|| receiver.recv().ok()) {
                    if matches!(request, Request::Scroll(_) | Request::Select(1, ..) | Request::FormulaPreview { .. }) {
                        while let Ok(next) = receiver.try_recv() {
                            if matches!((&request, &next), (Request::Scroll(_), Request::Scroll(_)) | (Request::Select(1, ..), Request::Select(1, ..)) | (Request::FormulaPreview { .. }, Request::FormulaPreview { .. })) { request = next; }
                            else { pending = Some(next); break; }
                        }
                    }
                    let mut inline_ready = None;
                    match request {
                        Request::Document {
                            generation,
                            cache_key,
                            source,
                            root,
                            width,
                            height,
                            scale,
                            theme,
                            font,
                            line_width, line_height, paragraph_spacing,
                            offset: y,
                        } => {
                            if serial.load(Ordering::Relaxed) != generation {
                                continue;
                            }
                            cancelled.store(false, Ordering::Relaxed);
                            if let (Some(old_key), Some(old_document)) =
                                (current_cache_key.take(), document.take())
                                && old_document.inline_rect().is_none()
                            {
                                // Only the newest revision/layout for a document is reusable.
                                cache.retain(|(key, _)| key.0 != old_key.0);
                                cache.push_front((
                                    old_key,
                                    CachedDocument {
                                        document: old_document,
                                        source: current_source.clone(),
                                        headings: current_headings.clone(),
                                        diagnostics: current_diagnostics.clone(),
                                    },
                                ));
                                cache.truncate(4);
                            }
                            if let Some(position) = cache.iter().position(|(key, _)| key == &cache_key)
                                && let Some((_, cached)) = cache.remove(position)
                            {
                                if trace {
                                    eprintln!("cache hit generation {generation}");
                                }
                                document = Some(cached.document);
                                current_source = cached.source;
                                current_headings = cached.headings;
                                current_diagnostics = cached.diagnostics;
                                current_generation = generation;
                                current_cache_key = Some(cache_key);
                                offset = y;
                                let headings = current_headings.clone();
                                let diagnostics = current_diagnostics.clone();
                                let check = serial.clone();
                                let _ = ui.upgrade_in_event_loop(move |ui| {
                                    if check.load(Ordering::Relaxed) != generation {
                                        return;
                                    }
                                    ui.set_headings(ModelRc::new(VecModel::from(
                                        headings
                                            .into_iter()
                                            .map(|(label, level, offset)| HeadingRow {
                                                label: label.into(),
                                                level,
                                                offset,
                                            })
                                            .collect::<Vec<_>>(),
                                    )));
                                    if !diagnostics.is_empty() {
                                        ui.set_notice(diagnostics.join("；").into());
                                    }
                                });
                            } else {
                            let result = (|| -> anyhow::Result<_> {
                                let rendered =
                                    engine.render(&source.to_string(), theme == 1, &cancelled)?;
                                if serial.load(Ordering::Relaxed) != generation {
                                    anyhow::bail!(tr("已取消"));
                                }
                                let html = wrap_html(
                                    &rendered.html,
                                    theme,
                                    font,
                                    line_width,
                                    &katex_css(&assets())?,
                                );
                                let html = html.replace("</head>", &format!("<style>body {{line-height:{line_height};}} p {{margin-bottom:{paragraph_spacing}px;}} ::selection {{background:#477ad9;color:white;}}</style></head>"));
                            let native = NativeDocument::new(
                                    &html,
                                    root,
                                    assets(),
                                    ((width as f32 * scale) as u32).clamp(1, 8192),
                                    ((height as f32 * scale) as u32).clamp(1, 8192),
                                    scale,
                                    theme == 1,
                                )?;
                                Ok((native, rendered))
                            })();
                            match result {
                                Ok((mut native, rendered)) => {
                                    if serial.load(Ordering::Relaxed) != generation {
                                        continue;
                                    }
                                    if let Some(switch) = inline_switch
                                        .take()
                                        .filter(|switch| switch.generation + 1 == generation)
                                    {
                                        // Editing can change every later byte offset. The active
                                        // block is the only mutable range, so its total delta maps
                                        // the target selected in the still-visible old layout.
                                        let delta = source.len_bytes() as isize
                                            - switch.old_source_len as isize;
                                        let target_start = if switch.target_start >= switch.active_end
                                        {
                                            switch.target_start.saturating_add_signed(delta)
                                        } else {
                                            switch.target_start
                                        };
                                        if let Some(target) = native.block_from_source(target_start)
                                        {
                                            let hit_x = target.x
                                                + switch
                                                    .local_x
                                                    .clamp(0., target.width.max(1.) - 1.);
                                            let hit_y = target.y
                                                + switch
                                                    .local_y
                                                    .clamp(0., target.height.max(1.) - 1.);
                                            let text_hit = native.text_at_point(hit_x, hit_y);
                                            if let Some(hit) =
                                                native.begin_inline_source(target_start)
                                            {
                                                let block = source
                                                    .byte_slice(hit.start..hit.end)
                                                    .to_string();
                                                let font = text_hit
                                                    .as_ref()
                                                    .map(|(_, _, font)| *font)
                                                    .unwrap_or(17.);
                                                let caret = text_hit
                                                    .map(|(text, byte, _)| {
                                                        hui_render::markdown::source_caret(
                                                            &block, &text, byte,
                                                        )
                                                    })
                                                    .unwrap_or(0);
                                                inline_ready = Some((font, caret, hit));
                                            }
                                        }
                                    }
                                    current_source = source;
                                    document = Some(native);
                                    current_generation = generation;
                                    current_cache_key = Some(cache_key);
                                    current_headings = rendered
                                        .headings
                                        .iter()
                                        .map(|heading| {
                                            (
                                                heading.title.clone(),
                                                heading.level as i32,
                                                heading.byte_offset as i32,
                                            )
                                        })
                                        .collect();
                                    current_diagnostics = rendered.diagnostics.clone();
                                    offset = y;
                                    let check = serial.clone();
                                    let _ = ui.upgrade_in_event_loop(move |ui| {
                                        if check.load(Ordering::Relaxed) != generation {
                                            return;
                                        }
                                        ui.set_headings(ModelRc::new(VecModel::from(
                                            rendered
                                                .headings
                                                .iter()
                                                .map(|h| HeadingRow {
                                                    label: h.title.clone().into(),
                                                    level: h.level as i32,
                                                    offset: h.byte_offset as i32,
                                                })
                                                .collect::<Vec<_>>(),
                                        )));
                                        if !rendered.diagnostics.is_empty() {
                                            ui.set_notice(rendered.diagnostics.join("；").into());
                                        }
                                    });
                                }
                                Err(e) => {
                                    if serial.load(Ordering::Relaxed) == generation {
                                        let message = message("排版未完成：{error}", &[("error", tr(&e.to_string()))]);
                                        let _ = ui.upgrade_in_event_loop(move |ui| {
                                            ui.set_notice(message.into());
                                            ui.invoke_action("render-failed".into());
                                            ui.set_busy(false);
                                            ui.window().request_redraw();
                                        });
                                    }
                                    continue;
                                }
                            }
                            }
                        }
                        Request::Scroll(y) => {
                            offset = y;
                        }
                        Request::Heading(byte) => {
                            if let Some(d) = document.as_mut() {
                                d.document.scroll_to_fragment(&format!("source-{byte}"));
                                offset = d.document.viewport_scroll().y as f32;
                                d.document.set_viewport_scroll(blitz_point_zero());
                            }
                        }
                        Request::Select(phase, x, y) => {
                            if serial.load(Ordering::Relaxed) != current_generation { continue; }
                            if let Some(d) = document.as_mut() { d.select_at(phase, x, y); }
                        }
                        Request::Copy => {
                            if serial.load(Ordering::Relaxed) == current_generation
                                && let Some(text) = document.as_ref().and_then(|d| d.selected_text()) {
                                let _ = ui.upgrade_in_event_loop(move |ui| ui.invoke_copy_ready(text.into()));
                            }
                            continue;
                        }
                        Request::PrepareInlineSwitch { generation, x, y } => {
                            if generation == current_generation
                                && let Some(d) = document.as_ref()
                                && let Some(hit) = d.block_at(x, y)
                                && let Some((_, active_end)) = d.inline_source_range()
                            {
                                inline_switch = Some(InlineSwitch {
                                    target_start: hit.start,
                                    local_x: x - hit.x,
                                    local_y: y - hit.y,
                                    active_end,
                                    old_source_len: current_source.len_bytes(),
                                    generation,
                                });
                            }
                            continue;
                        }
                        Request::FormulaPreview {
                            generation,
                            source,
                            width,
                            height,
                            scale,
                            theme,
                            font,
                        } => {
                            if generation != current_generation {
                                continue;
                            }
                            let result = (|| -> anyhow::Result<_> {
                                let rendered = engine.render(
                                    &source,
                                    theme == 1,
                                    &Arc::new(AtomicBool::new(false)),
                                )?;
                                let html = wrap_html(
                                    &rendered.html,
                                    theme,
                                    font,
                                    width,
                                    &katex_css(&assets())?,
                                )
                                .replace(
                                    "</head>",
                                    &format!(
                                        "<style>html,body{{width:100%;height:100%;overflow:hidden}} article.hui-document{{box-sizing:border-box;width:100%;height:{}px;max-width:none;padding:10px;display:flex;align-items:center;justify-content:center}} article.hui-document>.hui-block{{width:100%;margin:0}} .hui-math-display{{margin:0}}</style></head>",
                                        height.max(80.)
                                    ),
                                );
                                let mut native = NativeDocument::new(
                                    &html,
                                    None,
                                    assets(),
                                    ((width * scale) as u32).clamp(1, 8192),
                                    ((height * scale) as u32).clamp(1, 8192),
                                    scale,
                                    theme == 1,
                                )?;
                                let rgba = native.paint(0);
                                Ok((rgba, native.width, native.height))
                            })();
                            if let Ok((rgba, width, height)) = result {
                                let check = serial.clone();
                                let _ = ui.upgrade_in_event_loop(move |ui| {
                                    if check.load(Ordering::Relaxed) != generation
                                        || !ui.get_inline_math()
                                    {
                                        return;
                                    }
                                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                        &rgba, width, height,
                                    );
                                    ui.set_inline_formula_preview(
                                        slint::Image::from_rgba8_premultiplied(buffer),
                                    );
                                    ui.window().request_redraw();
                                });
                            }
                            continue;
                        }
                        Request::Hit(x, y) => {
                            if serial.load(Ordering::Relaxed) != current_generation {
                                continue;
                            }
                            if trace {
                                eprintln!("hit {x},{y}: {:?}", document.as_ref().and_then(|d| d.block_at(x,y)));
                            }
                            let text_hit = document.as_ref().and_then(|d| d.text_at_point(x,y));
                            if let Some(hit) = document.as_mut().and_then(|d| d.begin_inline(x, y)) {
                                let block = current_source.byte_slice(hit.start..hit.end).to_string();
                                let font = text_hit.as_ref().map(|(_,_,font)| *font).unwrap_or(17.);
                                let caret = text_hit.map(|(text, byte, _)| hui_render::markdown::source_caret(&block, &text, byte)).unwrap_or(0);
                                inline_ready = Some((font, caret, hit));
                            }
                        }
                    }
                    if let Some(d) = document.as_mut() {
                        if serial.load(Ordering::Relaxed) != current_generation {
                            continue;
                        }
                        let max = (d.content_height() - d.height as f32 / d.scale).max(0.);
                        offset = offset.clamp(0., max);
                        if trace {
                            eprintln!("paint generation {current_generation}");
                        }
                        let rgba = d.paint((offset * d.scale) as u32);
                        let (width, height) = (d.width, d.height);
                        let scale = d.scale;
                        let inline_rect = d.inline_rect();
                        let check = serial.clone();
                        let generation = current_generation;
                        let _ = ui.upgrade_in_event_loop(move |ui| {
                            if check.load(Ordering::Relaxed) != generation {
                                return;
                            }
                            if trace {
                                eprintln!("apply generation {generation}");
                            }
                            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                &rgba, width, height,
                            );
                            ui.set_preview(slint::Image::from_rgba8_premultiplied(buffer));
                            ui.set_frame_width(width as f32 / scale);
                            ui.set_frame_height(height as f32 / scale);
                            ui.set_displayed_mode(ui.get_mode());
                            if let Some((x,y,w)) = inline_rect {
                                ui.set_inline_x(x); ui.set_inline_y(y); ui.set_inline_width(w);
                            }
                            ui.set_preview_max(max);
                            ui.set_preview_position(offset);
                            ui.set_busy(false);
                            if let Some((font, caret, hit)) = inline_ready {
                                ui.invoke_inline_ready(
                                    font,
                                    caret as i32,
                                    hit.start as i32,
                                    hit.end as i32,
                                    hit.x,
                                    hit.y,
                                    hit.width,
                                    hit.height,
                                );
                            }
                            ui.window().request_redraw();
                        });
                    }
                }
            })
            .expect("layout worker");
        Self {
            sender,
            generation,
            cancel,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn document(
        &mut self,
        cache_key: RenderCacheKey,
        source: Rope,
        root: Option<PathBuf>,
        width: u32,
        height: u32,
        scale: f32,
        theme: i32,
        font: f32,
        line_width: f32,
        line_height: f32,
        paragraph_spacing: f32,
        offset: f32,
    ) {
        self.cancel.store(true, Ordering::Relaxed);
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.sender.send(Request::Document {
            generation,
            cache_key,
            source,
            root,
            width,
            height,
            scale,
            theme,
            font,
            line_width,
            line_height,
            paragraph_spacing,
            offset,
        });
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
    pub fn scroll(&self, y: f32) {
        let _ = self.sender.send(Request::Scroll(y));
    }
    pub fn heading(&self, byte: usize) {
        let _ = self.sender.send(Request::Heading(byte));
    }
    pub fn select(&self, phase: i32, x: f32, y: f32) {
        let _ = self.sender.send(Request::Select(phase, x, y));
    }
    pub fn copy(&self) {
        let _ = self.sender.send(Request::Copy);
    }
    pub fn prepare_inline_switch(&self, x: f32, y: f32) {
        let generation = self.generation.load(Ordering::Relaxed);
        let _ = self
            .sender
            .send(Request::PrepareInlineSwitch { generation, x, y });
    }
    pub fn formula_preview(
        &self,
        source: String,
        width: f32,
        height: f32,
        scale: f32,
        theme: i32,
        font: f32,
    ) {
        let generation = self.generation.load(Ordering::Relaxed);
        let _ = self.sender.send(Request::FormulaPreview {
            generation,
            source,
            width,
            height,
            scale,
            theme,
            font,
        });
    }
    pub fn hit(&self, x: f32, y: f32) {
        let _ = self.sender.send(Request::Hit(x, y));
    }
}

fn blitz_point_zero() -> hui_render::native::Point<f64> {
    hui_render::native::Point { x: 0., y: 0. }
}

pub fn export(source: &str, path: &Path, root: Option<PathBuf>, pdf: bool) -> anyhow::Result<()> {
    let mut engine = MarkdownEngine::new()?;
    engine.set_translator(tr);
    let result = engine.render(source, false, &Arc::new(AtomicBool::new(false)))?;
    if !result.diagnostics.is_empty() {
        anyhow::bail!(
            "{}: {}",
            tr("存在未完成渲染，未导出"),
            result.diagnostics.join("; ")
        );
    }
    if pdf {
        let html = wrap_html(&result.html, 0, 10.5, 510., &katex_css(&assets())?);
        hui_render::native::export_pdf(&html, path, root, assets())?;
    } else {
        // Inline font bytes so exported equations remain available offline and away from this installation.
        let mut css = include_str!("../../../vendor/katex/katex.min.css").to_string();
        use base64::Engine;
        for entry in std::fs::read_dir(assets().join("fonts"))? {
            let p = entry?.path();
            if p.extension().is_some_and(|s| s == "woff2") {
                let name = p.file_name().unwrap().to_string_lossy();
                let data = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&p)?);
                css = css.replace(
                    &format!("fonts/{name}"),
                    &format!("data:font/woff2;base64,{data}"),
                );
            }
        }
        let html = wrap_html(&result.html, 0, 17., 780., &css);
        hui_core::LocalStorage::save(path, &Rope::from_str(&html), None)?;
    }
    Ok(())
}

pub fn render_cli(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 2 {
        anyhow::bail!("用法: hui --render input.md output.png|output.html|output.pdf [dark|paper]");
    }
    let path = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let source = std::fs::read_to_string(&path)?;
    let root = path.canonicalize()?.parent().map(Path::to_path_buf);
    if output
        .extension()
        .is_some_and(|s| s == "pdf" || s == "html")
    {
        return export(&source, &output, root, output.extension().unwrap() == "pdf");
    }
    let mut engine = MarkdownEngine::new()?;
    engine.set_translator(tr);
    let theme = match args.get(2).map(String::as_str) {
        Some("dark") => 1,
        Some("paper") => 2,
        _ => 0,
    };
    let result = engine.render(&source, theme == 1, &Arc::new(AtomicBool::new(false)))?;
    let html = wrap_html(&result.html, theme, 17., 780., &katex_css(&assets())?);
    let mut native = NativeDocument::new(&html, root, assets(), 960, 1000, 1., theme == 1)?;
    write_png(&output, 960, 1000, &native.paint(0))?;
    println!(
        "{}; headings={}; diagnostics={:?}",
        output.display(),
        result.headings.len(),
        result.diagnostics
    );
    Ok(())
}
