//! Static UTF-8 TXT export; no formula/diagram renderer or network required.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

pub fn from_markdown(source: &str) -> String {
    let mut text = String::new();
    for event in Parser::new_ext(source, Options::all()) {
        match event {
            Event::Text(s) | Event::Code(s) | Event::InlineMath(s) | Event::DisplayMath(s) => {
                text.push_str(&s)
            }
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            Event::Rule => text.push_str("\n---\n"),
            Event::TaskListMarker(done) => text.push_str(if done { "[x] " } else { "[ ] " }),
            Event::Start(Tag::Item) => text.push_str("• "),
            Event::End(TagEnd::TableCell) => text.push('\t'),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::TableRow
                | TagEnd::Item,
            ) => {
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                if matches!(event, Event::End(TagEnd::Paragraph | TagEnd::Heading(_))) {
                    text.push('\n');
                }
            }
            Event::FootnoteReference(s) => {
                text.push('[');
                text.push_str(&s);
                text.push(']');
            }
            Event::Html(s) | Event::InlineHtml(s) => {
                let cleaned = ammonia::Builder::default()
                    .tags(std::collections::HashSet::new())
                    .clean(&s)
                    .to_string();
                text.push_str(&html_escape::decode_html_entities(&cleaned));
            }
            _ => {}
        }
    }
    let mut result = text.trim_end().to_string();
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_html_and_decodes_entities() {
        assert_eq!(
            from_markdown("<div>Hello &amp; <b>world</b></div>\n"),
            "Hello & world\n"
        );
        assert!(!from_markdown("<script>alert(1)</script>\n").contains("alert"));
    }
    #[test]
    fn strips_markup_preserves_multilingual_code_and_breaks() {
        let source = "# 标题\r\n\r\n**日本語** [link](https://example.com) ![图](x.png)\r\n\r\n```rust\r\nlet x = 1;\r\n```\r\n\r\n- [x] 한국어\r\n";
        let out = from_markdown(source);
        assert!(out.starts_with("标题\n\n日本語 link 图\n\n"));
        assert!(out.contains("let x = 1;\n"));
        assert!(out.contains("[x] 한국어"));
        assert!(!out.contains("```"));
        assert!(!out.contains('\r'));
    }
}
