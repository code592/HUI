use hui_core::{Document, Edit, LocalStorage, storage::Preferences};

#[test]
fn nine_languages_round_trip_edit_undo_and_crlf() {
    let text = include_str!("../../../packaging/smoke/multilingual.md").replace('\n', "\r\n");
    let mut document = Document::new(&text);
    let prefix =
        "简体 繁體 日本語 한국어 English Français Deutsch Русский Español 👩🏽‍💻 cafe\u{301}\r\n";
    document
        .apply(
            0,
            vec![Edit {
                range: 0..0,
                insert: prefix.into(),
            }],
        )
        .unwrap();
    assert_eq!(document.snapshot().to_string(), format!("{prefix}{text}"));
    assert!(document.undo());
    assert_eq!(document.snapshot().to_string().as_bytes(), text.as_bytes());
    assert!(document.redo());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("中文-日本語-한국어-Русский.md");
    LocalStorage::save(&path, &document.snapshot(), None).unwrap();
    let (loaded, _) = LocalStorage::read(&path).unwrap();
    assert_eq!(loaded.to_string(), format!("{prefix}{text}"));
}

#[test]
fn language_preference_survives_reload_without_changing_typography() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let mut prefs = Preferences::load(&path);
    assert!(prefs.language.is_empty());
    let size = prefs.source_font_size;
    for language in [
        "system", "zh-Hans", "zh-Hant", "en", "ja", "ko", "fr", "de", "ru", "es",
    ] {
        prefs.language = language.into();
        prefs.store(&path).unwrap();
        let loaded = Preferences::load(&path);
        assert_eq!(loaded.language, language);
        assert_eq!(loaded.source_font_size, size);
    }
}
