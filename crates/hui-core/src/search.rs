use crate::Edit;
use regex::RegexBuilder;
use ropey::Rope;
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};

pub fn find(
    text: &Rope,
    query: &str,
    regex: bool,
    case_sensitive: bool,
    cancel: &AtomicBool,
    limit: usize,
) -> Result<Vec<Range<usize>>, regex::Error> {
    if query.is_empty() {
        return Ok(vec![]);
    }
    let pattern = if regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    let expression = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .build()?;
    let snapshot = text.to_string();
    let mut results = Vec::new();
    for m in expression.find_iter(&snapshot) {
        if cancel.load(Ordering::Relaxed) || results.len() == limit {
            break;
        }
        results.push(text.byte_to_char(m.start())..text.byte_to_char(m.end()));
    }
    Ok(results)
}

pub fn replace_all(
    text: &Rope,
    query: &str,
    replacement: &str,
    regex: bool,
    case_sensitive: bool,
) -> Result<Vec<Edit>, regex::Error> {
    if query.is_empty() {
        return Ok(vec![]);
    }
    let pattern = if regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    let expression = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .build()?;
    let snapshot = text.to_string();
    Ok(expression
        .captures_iter(&snapshot)
        .map(|c| {
            let m = c.get(0).unwrap();
            let mut insert = String::new();
            if regex {
                c.expand(replacement, &mut insert);
            } else {
                insert.push_str(replacement);
            }
            Edit {
                range: text.byte_to_char(m.start())..text.byte_to_char(m.end()),
                insert,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_offsets_and_capture_replacement() {
        let r = Rope::from_str("中文 abc 中文");
        assert_eq!(
            find(&r, "中文", false, true, &AtomicBool::new(false), 10).unwrap(),
            vec![0..2, 7..9]
        );
        let edits = replace_all(&r, "(中)(文)", "$2$1", true, true).unwrap();
        assert_eq!(edits[0].insert, "文中");
    }
}
