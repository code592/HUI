use anyhow::{Context, Result};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};
use rquickjs::{Context as JsContext, Runtime};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

#[derive(Clone, Debug, Serialize)]
pub struct Heading {
    pub title: String,
    pub level: usize,
    pub byte_offset: usize,
}

pub struct RenderedMarkdown {
    pub html: String,
    pub headings: Vec<Heading>,
    pub diagnostics: Vec<String>,
}

pub struct MarkdownEngine {
    syntax: SyntaxSet,
    themes: ThemeSet,
    // The context must be dropped before its runtime; keep ownership together.
    js: JsContext,
    _runtime: Runtime,
    cache: HashMap<String, String>,
    math_loaded: bool,
    translate: fn(&str) -> String,
}

impl MarkdownEngine {
    pub fn new() -> Result<Self> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(128 * 1024 * 1024);
        runtime.set_max_stack_size(2 * 1024 * 1024);
        let js = JsContext::full(&runtime)?;
        js.with(|ctx| ctx.eval::<(), _>(include_str!("../../../vendor/katex/katex.min.js")))
            .context("初始化离线公式引擎失败")?;
        Ok(Self {
            syntax: SyntaxSet::load_defaults_newlines(),
            themes: ThemeSet::load_defaults(),
            js,
            _runtime: runtime,
            cache: HashMap::new(),
            math_loaded: false,
            translate: str::to_owned,
        })
    }
    /// Localizes engine-generated diagnostics only; never translates document text.
    pub fn set_translator(&mut self, translate: fn(&str) -> String) {
        self.translate = translate;
    }
    fn math(&mut self, source: &str, display: bool, dark: bool) -> Result<String> {
        let key = format!("math:{display}:{dark}:{source}");
        if let Some(result) = self.cache.get(&key) {
            return Ok(result.clone());
        }
        let expression = format!(
            "katex.renderToString({}, {{displayMode:{display},throwOnError:true,trust:false,strict:'error',maxExpand:1000}})",
            serde_json::to_string(source)?
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        self._runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
        let _validated: String = self
            .js
            .with(|ctx| ctx.eval(expression))
            .context((self.translate)("公式语法或资源限制错误"))?;
        // KaTeX's inline-table positioning is not yet rendered faithfully by Blitz.
        // Validate with the pinned KaTeX engine, then use a headless SVG layout for native display.
        if !self.math_loaded {
            self.js
                .with(|ctx| ctx.eval::<(), _>(include_str!("../../../vendor/mathjax/runtime.js")))
                .context((self.translate)("初始化原生 SVG 数学排版失败"))?;
            self.math_loaded = true;
        }
        #[derive(serde::Deserialize)]
        struct MathSvg {
            svg: String,
            width: f64,
            height: f64,
            align: f64,
        }
        let payload: String = self.js.with(|ctx| {
            ctx.eval(format!(
                "huiMathSvg({}, {display})",
                serde_json::to_string(source).unwrap()
            ))
        })?;
        let math: MathSvg = serde_json::from_str(&payload)?;
        if math.svg.contains("data-mjx-error") {
            anyhow::bail!((self.translate)("SVG 数学排版未能识别此公式"));
        }
        use base64::Engine;
        let svg = math
            .svg
            .replace("currentColor", if dark { "#e4e6ec" } else { "#2b303b" });
        let image = format!(
            "<img class=\"hui-math\" alt=\"{}\" src=\"data:image/svg+xml;base64,{}\" style=\"width:{}em;height:{}em;vertical-align:{}em;max-width:none\">",
            escape(source),
            base64::engine::general_purpose::STANDARD.encode(svg),
            math.width,
            math.height,
            math.align
        );
        let result = if display {
            format!("<div class=\"hui-math-display\">{image}</div>")
        } else {
            image
        };
        self.remember(key, result.clone());
        Ok(result)
    }
    fn diagram(&mut self, source: &str) -> Result<String> {
        use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
        let key = format!("mermaid:{source}");
        if let Some(result) = self.cache.get(&key) {
            return Ok(result.clone());
        }
        let output = Renderer::new().render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest::default(),
        ))?;
        let RenderOutput::Svg(Some(svg)) = output else {
            anyhow::bail!((self.translate)("没有可渲染的 Mermaid 图表"));
        };
        // Image decoding is handled by the native renderer, never by a browser.
        use base64::Engine;
        let result = format!(
            "<figure><img alt=\"Mermaid\" src=\"data:image/svg+xml;base64,{}\"></figure>",
            base64::engine::general_purpose::STANDARD.encode(svg.svg())
        );
        self.remember(key, result.clone());
        Ok(result)
    }
    fn remember(&mut self, key: String, value: String) {
        if self.cache.len() >= 64
            || self.cache.values().map(String::len).sum::<usize>() > 8 * 1024 * 1024
        {
            self.cache.clear();
        }
        self.cache.insert(key, value);
    }
    pub fn render(
        &mut self,
        source: &str,
        dark: bool,
        cancel: &Arc<AtomicBool>,
    ) -> Result<RenderedMarkdown> {
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_MATH
            | Options::ENABLE_HEADING_ATTRIBUTES;
        let mut events = Vec::new();
        let mut headings = Vec::new();
        let mut diagnostics = Vec::new();
        let mut code: Option<(String, String)> = None;
        let mut heading: Option<Heading> = None;
        let mut depth = 0usize;
        for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
            if cancel.load(Ordering::Relaxed) {
                anyhow::bail!((self.translate)("已取消"));
            }
            // Stable source anchors on semantic blocks support native hit-testing and inline editing.
            if matches!(event, Event::Start(_)) {
                if depth == 0 {
                    events.push(Event::Html(format!("<div class=\"hui-block\" data-source-start=\"{}\" data-source-end=\"{}\" id=\"block-{}\">", range.start, range.end, range.start).into()));
                }
                depth += 1;
            }
            let close_block = matches!(event, Event::End(_)) && depth == 1;
            if matches!(event, Event::End(_)) {
                depth = depth.saturating_sub(1);
            }
            match event {
                Event::Start(Tag::Paragraph) => events.push(Event::Html(
                    format!(
                        "<p data-source-start=\"{}\" data-source-end=\"{}\">",
                        range.start, range.end
                    )
                    .into(),
                )),
                Event::Start(Tag::Item) => events.push(Event::Html(
                    format!(
                        "<li data-source-start=\"{}\" data-source-end=\"{}\">",
                        range.start, range.end
                    )
                    .into(),
                )),
                Event::Start(Tag::CodeBlock(kind)) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(s) => s.into_string(),
                        _ => String::new(),
                    };
                    code = Some((lang, String::new()));
                }
                Event::Text(ref text) if code.is_some() => code.as_mut().unwrap().1.push_str(text),
                Event::End(TagEnd::CodeBlock) => {
                    let (lang, text) = code.take().unwrap();
                    let rendered = if lang.trim() == "mermaid" {
                        self.diagram(&text)
                    } else {
                        let token = lang.split_whitespace().next().unwrap_or("");
                        let syntax = self
                            .syntax
                            .find_syntax_by_token(token)
                            .unwrap_or_else(|| self.syntax.find_syntax_plain_text());
                        let theme = &self.themes.themes[if dark {
                            "base16-ocean.dark"
                        } else {
                            "InspiredGitHub"
                        }];
                        syntect::html::highlighted_html_for_string(
                            &text,
                            &self.syntax,
                            syntax,
                            theme,
                        )
                        .map_err(Into::into)
                    };
                    match rendered {
                        Ok(result) => events.push(Event::Html(result.into())),
                        Err(err) => {
                            diagnostics.push(format!(
                                "{} {}: {err}",
                                (self.translate)("字节"),
                                range.start
                            ));
                            events.push(Event::Html(
                                format!(
                                    "<aside class=\"render-error\">{}: {}</aside><pre>{}</pre>",
                                    escape(&(self.translate)("图表渲染未完成")),
                                    escape(&err.to_string()),
                                    escape(&text)
                                )
                                .into(),
                            ));
                        }
                    }
                }
                Event::InlineMath(ref value) | Event::DisplayMath(ref value) => {
                    let display = matches!(event, Event::DisplayMath(_));
                    match self.math(value, display, dark) {
                        Ok(result) => events.push(Event::Html(result.into())),
                        Err(err) => {
                            diagnostics.push(format!(
                                "{} {}: {err}",
                                (self.translate)("字节"),
                                range.start
                            ));
                            events.push(Event::Html(
                                format!(
                                    "<span class=\"render-error\">{}: {}</span>",
                                    escape(&(self.translate)("公式渲染未完成")),
                                    escape(value)
                                )
                                .into(),
                            ));
                        }
                    }
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    heading = Some(Heading {
                        title: String::new(),
                        level: level as usize,
                        byte_offset: range.start,
                    });
                    events.push(Event::Html(
                        format!(
                            "<h{} id=\"source-{}\" data-source-start=\"{}\" data-source-end=\"{}\">",
                            level as usize,
                            range.start,
                            range.start,
                            range.end
                        )
                        .into(),
                    ));
                }
                Event::End(TagEnd::Heading(level)) => {
                    if let Some(h) = heading.take() {
                        headings.push(h);
                    }
                    events.push(Event::Html(format!("</h{}>", level as usize).into()));
                }
                Event::Text(ref text) | Event::Code(ref text) => {
                    if let Some(h) = heading.as_mut() {
                        h.title.push_str(text);
                    }
                    events.push(event);
                }
                Event::Html(raw) | Event::InlineHtml(raw) => {
                    // Preserve static styling; all resource fetches are separately sandboxed.
                    let cleaned = ammonia::Builder::default()
                        .add_tags(&[
                            "style",
                            "details",
                            "summary",
                            "section",
                            "article",
                            "figure",
                            "figcaption",
                        ])
                        .rm_clean_content_tags(&["style"])
                        .add_generic_attributes(&["style", "class", "id"])
                        .clean(&raw)
                        .to_string();
                    events.push(Event::Html(cleaned.into()));
                }
                _ => events.push(event),
            }
            if close_block {
                // Start-tag offset ranges cover the entire Markdown block in pulldown-cmark.
                events.push(Event::Html("</div>".into()));
            }
        }
        let mut result = String::new();
        html::push_html(&mut result, events.into_iter());
        Ok(RenderedMarkdown {
            html: result,
            headings,
            diagnostics,
        })
    }
}

/// Map a native inline text caret back through Markdown markup to a UTF-8 source boundary.
/// The lookup is limited to the clicked block, never the full document.
pub fn source_caret(source: &str, inline_text: &str, offset: usize) -> usize {
    let mut chars: Vec<(char, usize, usize)> = Vec::new();
    for (event, range) in Parser::new_ext(source, Options::all()).into_offset_iter() {
        match event {
            Event::Text(text) | Event::Code(text) => {
                let raw = &source[range.clone()];
                if let Some(base) = raw.find(text.as_ref()) {
                    for (byte, ch) in text.char_indices() {
                        chars.push((
                            ch,
                            range.start + base + byte,
                            range.start + base + byte + ch.len_utf8(),
                        ));
                    }
                } else {
                    // Entities and escaped punctuation are separate parser events.
                    for ch in text.chars() {
                        chars.push((ch, range.start, range.end));
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => chars.push((' ', range.start, range.end)),
            _ => {}
        }
    }
    let normalize = |ch: char| if ch.is_whitespace() { ' ' } else { ch };
    let text: Vec<char> = chars.iter().map(|(c, _, _)| normalize(*c)).collect();
    let needle: Vec<char> = inline_text.chars().map(normalize).collect();
    let count = inline_text
        .char_indices()
        .take_while(|(i, _)| *i < offset)
        .count();
    let start = if needle.is_empty() {
        0
    } else {
        text.windows(needle.len())
            .position(|part| part == needle)
            .unwrap_or(0)
    };
    let at = start + count;
    chars
        .get(at)
        .map(|(_, before, _)| *before)
        .or_else(|| chars.last().map(|(_, _, after)| *after))
        .unwrap_or(0)
        .min(source.len())
}

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn stylesheet(theme: i32, font: f32, width: f32) -> String {
    let (paper, ink, muted, border, code) = match theme {
        1 => ("#1c1e23", "#e4e6ec", "#a1a8b5", "#343942", "#252a32"),
        2 => ("#f5efdf", "#393a34", "#787768", "#ded6c4", "#ebe3d2"),
        _ => ("#fdfcf9", "#2b303b", "#77808d", "#e8e9ec", "#f2f4f7"),
    };
    format!(
        r#"html,body{{margin:0;background:{paper};color:{ink};}} body{{font-family:system-ui,-apple-system,'Segoe UI','PingFang SC','PingFang TC','Hiragino Sans','Yu Gothic','Malgun Gothic','Noto Sans CJK SC','Noto Sans CJK TC','Noto Sans CJK JP','Noto Sans CJK KR',sans-serif;font-size:{font}px;line-height:1.5;}} article.hui-document{{max-width:{width}px;margin:0 auto;padding:24px 28px 48px;overflow-wrap:anywhere;}}h1{{font-size:1.8em;line-height:1.25;letter-spacing:-.025em;margin:0 0 16px;font-weight:700;}}h2{{font-size:1.45em;margin-top:24px;margin-bottom:12px;}}h3{{font-size:1.16em;margin-top:20px;margin-bottom:10px;}}p{{margin:0 0 12px;}}a{{color:#477ad9;text-decoration:none;}}blockquote{{margin:12px 0;padding:0 14px;border-left:3px solid #83a8e8;color:{muted};}}pre{{margin:12px 0;padding:12px;border-radius:8px;overflow:auto;background:{code};font-size:.9em;line-height:1.5;}}code{{font-family:Menlo,Consolas,'Liberation Mono',monospace;}}:not(pre)>code{{background:{code};padding:2px 5px;border-radius:4px;font-size:.88em;}}hr{{border:0;border-top:1px solid {border};margin:20px 0;}}table{{border-collapse:separate;border-spacing:0;width:100%;margin:12px 0;}}th,td{{border-bottom:1px solid {border};padding:6px 10px;text-align:left;}}th{{background:{code};}}img,svg{{max-width:100%;height:auto;}}figure{{margin:16px 0;}}ul,ol{{margin:0 0 12px;padding-left:26px;}}ul ul,ul ol,ol ul,ol ol{{margin-bottom:0;}}li{{margin:2px 0;}}li>p{{margin-bottom:6px;}}.render-error{{color:#b45138;background:#fceae2;padding:8px;}}.hui-document .katex .vlist-t{{border-collapse:separate;border-spacing:0;}}.hui-math-display{{text-align:center;margin:16px 0;overflow:auto;}}.katex-display{{overflow:auto;padding:8px 0;}}"#
    )
}

pub fn wrap_html(body: &str, theme: i32, font: f32, width: f32, katex_css: &str) -> String {
    format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>{}</style><style>{katex_css}</style></head><body><article class=\"hui-document\">{body}</article></body></html>",
        stylesheet(theme, font, width)
    )
}

/// Color-only source spans preserve the native input's glyph metrics and every source byte.
/// Character references keep literal Markdown (including newlines) in one styled paragraph.
pub fn highlighted_source(source: &str, dark: bool) -> String {
    use std::fmt::Write;
    use syntect::{easy::HighlightLines, util::LinesWithEndings};
    static DATA: std::sync::OnceLock<(SyntaxSet, ThemeSet)> = std::sync::OnceLock::new();
    let (syntax, themes) = DATA.get_or_init(|| {
        (
            SyntaxSet::load_defaults_newlines(),
            ThemeSet::load_defaults(),
        )
    });
    let grammar = syntax
        .find_syntax_by_extension("md")
        .unwrap_or_else(|| syntax.find_syntax_plain_text());
    let theme = &themes.themes[if dark {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    }];
    let mut highlighter = HighlightLines::new(grammar, theme);
    let mut out = String::new();
    for line in LinesWithEndings::from(source) {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        let heading =
            (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(char::is_whitespace);
        let spans = highlighter.highlight_line(line, syntax).unwrap_or_default();
        if spans.is_empty() && !line.is_empty() {
            return String::new();
        }
        for (style, text) in spans {
            let c = if heading {
                syntect::highlighting::Color {
                    r: if dark { 137 } else { 58 },
                    g: if dark { 177 } else { 102 },
                    b: if dark { 241 } else { 170 },
                    a: 255,
                }
            } else {
                let c = style.foreground;
                let punctuation = text
                    .chars()
                    .all(|ch| ch.is_ascii_punctuation() || ch.is_whitespace());
                if punctuation && c.r > c.g {
                    syntect::highlighting::Color {
                        r: if dark { 154 } else { 120 },
                        g: if dark { 163 } else { 130 },
                        b: if dark { 180 } else { 147 },
                        a: 255,
                    }
                } else if c.r as u16 > c.g as u16 * 3 / 2 && c.r as u16 > c.b as u16 * 3 / 2 {
                    // The stock Markdown theme paints entire lists orange. Keep body text quiet.
                    syntect::highlighting::Color {
                        r: if dark { 224 } else { 43 },
                        g: if dark { 227 } else { 48 },
                        b: if dark { 234 } else { 59 },
                        a: 255,
                    }
                } else {
                    c
                }
            };
            let _ = write!(out, "<font color=\"#{:02x}{:02x}{:02x}\">", c.r, c.g, c.b);
            for ch in text.chars() {
                let _ = write!(out, "&#{};", ch as u32);
            }
            out.push_str("</font>");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_colors_preserve_literal_markdown_and_line_endings() {
        let source = "# 标题\r\n\r\n**粗体** & <tag>\t😀\n```rust\nlet x = 1;\n```\n";
        for dark in [false, true] {
            let markup = highlighted_source(source, dark);
            let restored: String = Parser::new(&markup)
                .filter_map(|event| match event {
                    Event::Text(text) => Some(text.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(restored, source);
            assert!(markup.contains("<font color="));
        }
    }
    #[test]
    fn clicked_text_maps_through_unicode_and_markdown_delimiters() {
        let source = "欢迎 **中文😀** 与 [链接](https://example.com) 和 `code`。\r\n";
        let visible = "欢迎 中文😀 与 链接 和 code。";
        for word in ["中文", "😀", "链接", "code", "。"] {
            let offset = visible.find(word).unwrap();
            let mapped = source_caret(source, visible, offset);
            assert_eq!(mapped, source.find(word).unwrap(), "{word}");
            assert!(source.is_char_boundary(mapped));
        }
        let source = "# Title &amp; **tail**\n";
        assert_eq!(
            source_caret(source, "Title & tail", 8),
            source.find("tail").unwrap()
        );
        assert_eq!(source_caret("a", "much longer text", 200), 1);
    }
    #[test]
    fn gfm_formula_and_source_mapping() {
        let mut e = MarkdownEngine::new().unwrap();
        let result = e
            .render(
                "# 中文\n\n- [x] done\n\n$x^2$\n\n| A | B |\n|---|---|\n| 1 | 2 |",
                false,
                &Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
        assert!(result.html.contains("<table>"));
        assert!(result.html.contains("hui-math"));
        assert_eq!(result.headings[0].title, "中文");
        assert!(result.diagnostics.is_empty());
    }
    #[test]
    fn untrusted_scripts_removed() {
        let mut e = MarkdownEngine::new().unwrap();
        let result = e
            .render(
                "<script>alert(1)</script>\n\n<img src=\"x\" onerror=\"alert(1)\">",
                false,
                &Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
        assert!(!result.html.contains("<script"));
        assert!(!result.html.contains("onerror"));
    }
}
