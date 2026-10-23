//! Mermaid syntax is rendered natively. Browser-only labels are normalized before painting.
use anyhow::{Context, Result};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
    svg::{RootBackgroundPostprocessor, SvgPipeline},
};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

fn safe_svg(source: &str, dark: bool) -> Result<String> {
    let config = MermaidConfig::from_value(serde_json::json!({
        "theme": if dark { "dark" } else { "default" },
        "securityLevel": "strict",
        "fontFamily": "Arial, Noto Sans CJK SC, PingFang SC, Noto Sans, sans-serif"
    }));
    let request = SvgRequest {
        // The parity renderer starts with a white canvas even for dark diagrams.
        // Let the native document's paper color show through in every theme.
        pipeline: Some(
            SvgPipeline::resvg_safe()
                .with_postprocessor(RootBackgroundPostprocessor::new("transparent")),
        ),
        ..Default::default()
    };
    let result = Renderer::new()
        .with_engine(Engine::new().with_site_config(config))
        .render(RenderRequest::svg(
            source,
            OperationControl::new().with_deadline(Duration::from_secs(5)),
            request,
        ))?;
    let RenderOutput::Svg(Some(svg)) = result else {
        anyhow::bail!("No Mermaid diagram detected");
    };
    Ok(svg.svg().to_owned())
}

fn fonts() -> Result<Arc<usvg::fontdb::Database>> {
    static FONTS: OnceLock<std::result::Result<Arc<usvg::fontdb::Database>, String>> =
        OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            // fontdb's system discovery explicitly excludes Android. Read the platform
            // fonts ourselves; otherwise SVG text is silently dropped during outlining.
            #[cfg(target_os = "android")]
            {
                db.load_fonts_dir("/system/fonts");
                db.load_fonts_dir("/product/fonts");
                db.set_sans_serif_family("Roboto");
                db.set_serif_family("Noto Serif");
                db.set_monospace_family("Droid Sans Mono");
            }
            #[cfg(target_os = "windows")]
            db.load_font_source(usvg::fontdb::Source::Binary(
                crate::native::windows_font_bytes().map_err(|error| error.to_string())?,
            ));
            if db.is_empty() {
                return Err("No fonts available for Mermaid labels".to_owned());
            }
            Ok(Arc::new(db))
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

pub(crate) fn render(source: &str, dark: bool) -> Result<String> {
    let svg = safe_svg(source, dark)?;
    let options = usvg::Options {
        fontdb: fonts()?,
        image_href_resolver: usvg::ImageHrefResolver {
            // Diagrams cannot use their labels/images to read ambient files or fetch URLs.
            resolve_string: Box::new(|_, _| None),
            ..Default::default()
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg, &options).context("Mermaid SVG layout failed")?;
    // Bake glyph outlines once, so the native view and offline HTML/PDF exports use the
    // same labels even when the consumer has a different SVG font database. Still vector.
    let output = tree.to_string(&usvg::WriteOptions::default());
    // Native data-image input is capped at 32 MiB including base64 expansion.
    anyhow::ensure!(
        output.len() <= 24 * 1024 * 1024,
        "Mermaid SVG exceeds the native image limit"
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> std::path::PathBuf {
        std::env::var_os("HUI_RENDER_TEST_ASSETS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/katex")
            })
    }

    const CASES: &[(&str, &str)] = &[
        ("graph", "graph TD\n A[One] --> B[Two]"),
        (
            "flowchart",
            "flowchart LR\n A[开始] --> B{保存?}\n B -->|是| C[完成]",
        ),
        (
            "sequence",
            "sequenceDiagram\n participant U as 用户\n participant A as 应用\n U->>A: 打开文档\n A-->>U: 完成",
        ),
        (
            "class",
            "classDiagram\n Animal <|-- Duck\n Animal : +String name\n Duck : +swim()",
        ),
        (
            "state",
            "stateDiagram-v2\n [*] --> Editing\n Editing --> Saved: save\n Saved --> [*]",
        ),
        (
            "er",
            "erDiagram\n USER ||--o{ DOCUMENT : writes\n USER {\n string name\n }",
        ),
        (
            "gantt",
            "gantt\n title Release\n dateFormat YYYY-MM-DD\n section Desktop\n Build :a1, 2026-10-01, 2d\n Test :after a1, 2d",
        ),
        (
            "pie",
            "pie title Documents\n \"Markdown\" : 70\n \"Other\" : 30",
        ),
        (
            "journey",
            "journey\n title Writing\n section Draft\n Open: 5: User\n Edit: 4: User",
        ),
        ("mindmap", "mindmap\n root((HUI))\n  Read\n  Write"),
        (
            "timeline",
            "timeline\n title Roadmap\n 2026 : Desktop\n 2027 : Mobile",
        ),
        (
            "gitGraph",
            "gitGraph\n commit\n branch develop\n checkout develop\n commit\n checkout main\n merge develop",
        ),
    ];

    #[test]
    fn common_diagrams_have_native_vector_output() {
        for &(name, source) in CASES {
            let svg = render(source, false).unwrap_or_else(|error| panic!("{name}: {error:#}"));
            assert!(svg.contains("<path"), "{name}: no vector content");
            assert!(!svg.contains("foreignObject"), "{name}: browser-only label");
            assert!(!svg.contains("<text"), "{name}: unembedded font dependency");
            assert!(!svg.contains("<script"), "{name}: executable content");
        }
    }

    #[test]
    fn common_diagrams_paint_in_the_native_document_engine() {
        use crate::{
            markdown::{MarkdownEngine, wrap_html},
            native::NativeDocument,
        };
        use std::sync::atomic::AtomicBool;
        let mut engine = MarkdownEngine::new().unwrap();
        let assets = assets();
        for &(name, source) in CASES {
            let result = engine
                .render(
                    &format!("```mermaid\n{source}\n```"),
                    false,
                    &Arc::new(AtomicBool::new(false)),
                )
                .unwrap();
            assert!(
                result.diagnostics.is_empty(),
                "{name}: {:?}",
                result.diagnostics
            );
            let html = wrap_html(&result.html, 0, 17., 580., "");
            let mut doc =
                NativeDocument::new(&html, None, assets.clone(), 640, 640, 1., false).unwrap();
            let pixels = doc.paint(0);
            if let Some(directory) = std::env::var_os("HUI_RENDER_TEST_OUTPUT") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                crate::native::write_png(&directory.join(format!("{name}.png")), 640, 640, &pixels)
                    .unwrap();
            }
            let ink = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] < 180 && p[1] < 180 && p[2] < 180 && p[3] > 0)
                .count();
            assert!(ink > 100, "{name}: native document lost the diagram");
        }
    }

    #[test]
    fn html_labels_survive_native_conversion_and_theme_changes() {
        let source = "flowchart LR\n A[\"开始<br/>第二行\"] --> B[保存]";
        let safe = safe_svg(source, false).unwrap();
        assert!(safe.contains("开始") && safe.contains("第二行") && safe.contains("保存"));
        assert!(!safe.contains("foreignObject"));
        let light = render(source, false).unwrap();
        let dark = render(source, true).unwrap();
        assert_ne!(light, dark, "dark theme must change graph colors");
        assert!(
            light.matches("<path").count() > 8,
            "labels lost their glyph outlines"
        );
    }

    #[test]
    fn dark_diagram_canvas_preserves_document_background() {
        use crate::{markdown::wrap_html, native::NativeDocument};
        use base64::Engine as _;
        let svg = render("sequenceDiagram\n A->>B: Hello", true).unwrap();
        let html = wrap_html(
            &format!(
                "<img src=\"data:image/svg+xml;base64,{}\">",
                base64::engine::general_purpose::STANDARD.encode(svg)
            ),
            1,
            17.,
            580.,
            "",
        );
        let assets = assets();
        let mut doc = NativeDocument::new(&html, None, assets, 640, 640, 1., true).unwrap();
        let pixels = doc.paint(0);
        let white_pixels = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 245 && p[1] > 245 && p[2] > 245 && p[3] > 0)
            .count();
        assert!(white_pixels < 1000, "dark diagram gained a white canvas");
    }

    #[test]
    fn invalid_diagrams_report_errors() {
        assert!(render("not-a-diagram\n hello", false).is_err());
        assert!(render("flowchart LR\n A[broken", false).is_err());
    }
}
