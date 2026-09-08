//! Shared, bundled UI catalogs. Document text never passes through this module.
use crate::{AppWindow, I18n};
use slint::ComponentHandle;
use std::{
    collections::BTreeMap,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

pub const LANGUAGES: [&str; 9] = [
    "zh-Hans", "zh-Hant", "en", "ja", "ko", "fr", "de", "ru", "es",
];
const JSON: [&str; 9] = [
    include_str!("../assets/locales/zh-Hans.json"),
    include_str!("../assets/locales/zh-Hant.json"),
    include_str!("../assets/locales/en.json"),
    include_str!("../assets/locales/ja.json"),
    include_str!("../assets/locales/ko.json"),
    include_str!("../assets/locales/fr.json"),
    include_str!("../assets/locales/de.json"),
    include_str!("../assets/locales/ru.json"),
    include_str!("../assets/locales/es.json"),
];
static CATALOGS: OnceLock<Vec<BTreeMap<String, String>>> = OnceLock::new();
static CURRENT: AtomicUsize = AtomicUsize::new(2);
fn catalogs() -> &'static Vec<BTreeMap<String, String>> {
    CATALOGS.get_or_init(|| {
        JSON.iter()
            .map(|s| serde_json::from_str(s).expect("valid bundled locale"))
            .collect()
    })
}
pub fn resolve(tag: &str) -> usize {
    let tag = tag.replace('_', "-").to_ascii_lowercase();
    let parts: Vec<_> = tag.split(['-', '.', '@']).collect();
    if parts.first() == Some(&"zh") {
        return usize::from(
            parts.contains(&"hant")
                || (!parts.contains(&"hans")
                    && parts.iter().any(|p| ["tw", "hk", "mo"].contains(p))),
        );
    }
    LANGUAGES
        .iter()
        .position(|l| Some(&l.to_ascii_lowercase().as_str()) == parts.first())
        .unwrap_or(2)
}
pub fn lookup(source: &str, index: usize) -> String {
    catalogs()
        .get(index)
        .and_then(|c| c.get(source))
        .cloned()
        .unwrap_or_else(|| source.to_owned())
}
pub fn tr(source: &str) -> String {
    lookup(source, CURRENT.load(Ordering::Relaxed))
}
/// Named placeholders are replaced in a single pass so inserted filenames stay literal.
pub fn message(source: &str, args: &[(&str, String)]) -> String {
    let template = tr(source);
    let mut result = String::with_capacity(template.len());
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            result.push_str(&rest[start..]);
            return result;
        };
        let key = &rest[start + 1..start + end];
        if let Some((_, value)) = args.iter().find(|(name, _)| *name == key) {
            result.push_str(value);
        } else {
            result.push_str(&rest[start..=start + end]);
        }
        rest = &rest[start + end + 1..];
    }
    result.push_str(rest);
    result
}
pub fn apply(ui: &AppWindow, preference: &str) {
    let language = if preference.is_empty() || preference == "system" {
        // HUI_LANGUAGE is also useful for deterministic screenshots and packaging QA.
        std::env::var("HUI_LANGUAGE")
            .ok()
            .or_else(sys_locale::get_locale)
            .unwrap_or_else(|| "en".into())
    } else {
        preference.to_owned()
    };
    let index = resolve(&language);
    CURRENT.store(index, Ordering::Relaxed);
    ui.global::<I18n>().set_locale(index as i32);
    ui.set_language_index(
        LANGUAGES
            .iter()
            .position(|l| *l == preference)
            .map_or(0, |i| i as i32 + 1),
    );
}
pub fn install(ui: &AppWindow, preference: &str) {
    ui.global::<I18n>()
        .on_translate(|source, locale| lookup(&source, locale as usize).into());
    apply(ui, preference);
}
pub fn welcome() -> &'static str {
    const DOCS: [&str; 9] = [
        include_str!("../assets/locales/welcome-zh-Hans.md"),
        include_str!("../assets/locales/welcome-zh-Hant.md"),
        include_str!("../assets/locales/welcome-en.md"),
        include_str!("../assets/locales/welcome-ja.md"),
        include_str!("../assets/locales/welcome-ko.md"),
        include_str!("../assets/locales/welcome-fr.md"),
        include_str!("../assets/locales/welcome-de.md"),
        include_str!("../assets/locales/welcome-ru.md"),
        include_str!("../assets/locales/welcome-es.md"),
    ];
    DOCS[CURRENT.load(Ordering::Relaxed)]
}

pub fn current() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_matching() {
        for tag in ["zh-TW", "zh_HK.UTF-8", "zh-Hant", "zh-MO"] {
            assert_eq!(resolve(tag), 1);
        }
        for tag in ["zh", "zh-CN", "zh-Hans-TW"] {
            assert_eq!(resolve(tag), 0);
        }
        for (tag, n) in [
            ("en-US", 2),
            ("ja-JP", 3),
            ("ko_KR", 4),
            ("fr-CA", 5),
            ("de-DE", 6),
            ("ru-RU", 7),
            ("es-MX", 8),
            ("it-IT", 2),
        ] {
            assert_eq!(resolve(tag), n);
        }
    }
    #[test]
    fn catalogs_complete() {
        let base = &catalogs()[0];
        for catalog in catalogs() {
            assert_eq!(
                base.keys().collect::<Vec<_>>(),
                catalog.keys().collect::<Vec<_>>()
            );
            for (key, value) in catalog {
                assert!(!value.trim().is_empty(), "{key}");
            }
        }
    }
}
