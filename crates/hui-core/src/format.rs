//! Markdown commands operate on source character offsets, shared by both editing views.
use crate::{Edit, Selection};
use ropey::Rope;
use std::ops::Range;

pub struct FormatEdit {
    pub edit: Edit,
    pub selection: Selection,
}

pub fn command(source: &Rope, range: Range<usize>, name: &str) -> Option<FormatEdit> {
    if range.end > source.len_chars() || range.start > range.end {
        return None;
    }
    let pairs = match name {
        "bold" => Some(("**", "**")),
        "italic" => Some(("*", "*")),
        "strike" => Some(("~~", "~~")),
        "code" => Some(("`", "`")),
        "link" => Some(("[", "](https://)")),
        _ => None,
    };
    if let Some((left, right)) = pairs {
        let text = source.slice(range.clone()).to_string();
        let (l, r) = (left.chars().count(), right.chars().count());
        if text.len() >= left.len() + right.len() && text.starts_with(left) && text.ends_with(right)
        {
            let insert = text[left.len()..text.len() - right.len()].to_string();
            let selection = Selection {
                anchor: range.start,
                head: range.start + insert.chars().count(),
            };
            return Some(FormatEdit {
                edit: Edit { range, insert },
                selection,
            });
        }
        if range.start >= l
            && range.end + r <= source.len_chars()
            && source.slice(range.start - l..range.start) == left
            && source.slice(range.end..range.end + r) == right
        {
            let selection = Selection {
                anchor: range.start - l,
                head: range.end - l,
            };
            return Some(FormatEdit {
                edit: Edit {
                    range: range.start - l..range.end + r,
                    insert: text,
                },
                selection,
            });
        }
        let selection = Selection {
            anchor: range.start + l,
            head: range.end + l,
        };
        return Some(FormatEdit {
            edit: Edit {
                range,
                insert: format!("{left}{text}{right}"),
            },
            selection,
        });
    }
    let first = source.char_to_line(range.start);
    let last = source.char_to_line(if range.end > range.start {
        range.end - 1
    } else {
        range.end
    });
    let start = source.line_to_char(first);
    let end = if last + 1 < source.len_lines() {
        source.line_to_char(last + 1)
    } else {
        source.len_chars()
    };
    let text = source.slice(start..end).to_string();
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if name == "code-block" {
        let content = text.trim_end_matches(['\r', '\n']);
        let insert = format!("```{newline}{content}{newline}```{newline}");
        let anchor = start + 3 + newline.len();
        return Some(FormatEdit {
            edit: Edit {
                range: start..end,
                insert,
            },
            selection: Selection {
                anchor,
                head: anchor + content.chars().count(),
            },
        });
    }
    let prefix = match name {
        "heading" | "h2" => "## ",
        "h1" => "# ",
        "h3" => "### ",
        "h4" => "#### ",
        "h5" => "##### ",
        "h6" => "###### ",
        "task" => "- [ ] ",
        "list" => "- ",
        "ordered" => "1. ",
        "quote" => "> ",
        _ => return None,
    };
    let mut lines = text.split_inclusive('\n').collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push("");
    }
    let remove = lines.iter().all(|line| line.starts_with(prefix));
    let insert = lines
        .iter()
        .map(|line| {
            if remove {
                line[prefix.len()..].to_string()
            } else {
                let line = if name.starts_with('h') {
                    let n = line.chars().take_while(|&c| c == '#').count();
                    if (1..=6).contains(&n) && line.as_bytes().get(n) == Some(&b' ') {
                        &line[n + 1..]
                    } else {
                        line
                    }
                } else {
                    line
                };
                format!("{prefix}{line}")
            }
        })
        .collect::<String>();
    let head = start + if remove { 0 } else { prefix.chars().count() };
    Some(FormatEdit {
        edit: Edit {
            range: start..end,
            insert,
        },
        selection: Selection { anchor: head, head },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_wrap_toggle_and_caret() {
        let rope = Rope::from_str("中文 emoji 👩‍💻");
        let r = command(&rope, 0..2, "bold").unwrap();
        assert_eq!(r.edit.insert, "**中文**");
        assert_eq!((r.selection.anchor, r.selection.head), (2, 4));
        let r = command(&Rope::from_str("**中文**"), 2..4, "bold").unwrap();
        assert_eq!(r.edit.range, 0..6);
        assert_eq!(r.edit.insert, "中文");
        let r = command(&Rope::new(), 0..0, "code").unwrap();
        assert_eq!(r.edit.insert, "``");
        assert_eq!(r.selection.head, 1);
    }
    #[test]
    fn block_commands_preserve_crlf_and_expand_selected_lines() {
        let rope = Rope::from_str("一\r\n二\r\n三\r\n");
        let r = command(&rope, 1..5, "list").unwrap();
        assert_eq!(r.edit.insert, "- 一\r\n- 二\r\n");
        assert_eq!(r.edit.range, 0..6);
        let r = command(&Rope::from_str("### 已有标题\n"), 4..4, "h1").unwrap();
        assert_eq!(r.edit.insert, "# 已有标题\n");
    }
}
