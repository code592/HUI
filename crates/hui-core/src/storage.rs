use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStamp(pub blake3::Hash);

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("文件已被其他应用修改；已保留当前编辑，请另存为或重新打开")]
    Conflict,
    #[error("文件不是有效 UTF-8，未修改原文件")]
    Encoding,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct LocalStorage;
impl LocalStorage {
    pub fn read(path: &Path) -> Result<(Rope, FileStamp), StorageError> {
        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| StorageError::Encoding)?;
        Ok((Rope::from_str(text), FileStamp(blake3::hash(&bytes))))
    }
    pub fn stamp(path: &Path) -> Result<FileStamp, StorageError> {
        let mut f = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0; 65536];
        loop {
            let n = f.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(FileStamp(hasher.finalize()))
    }
    pub fn save(
        path: &Path,
        text: &Rope,
        expected: Option<&FileStamp>,
    ) -> Result<FileStamp, StorageError> {
        let path = if path.is_symlink() {
            fs::canonicalize(path)?
        } else {
            path.to_path_buf()
        };
        if let Some(stamp) = expected
            && Self::stamp(&path).ok().as_ref() != Some(stamp)
        {
            return Err(StorageError::Conflict);
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.permissions().readonly() {
                return Err(
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "文件只读").into(),
                );
            }
            tmp.as_file().set_permissions(metadata.permissions())?;
        }
        let mut hasher = blake3::Hasher::new();
        for chunk in text.chunks() {
            tmp.write_all(chunk.as_bytes())?;
            hasher.update(chunk.as_bytes());
        }
        tmp.as_file().sync_all()?;
        // Recheck just before replacement. External applications cannot be forced to honor advisory locks.
        if let Some(stamp) = expected
            && Self::stamp(&path).ok().as_ref() != Some(stamp)
        {
            return Err(StorageError::Conflict);
        }
        tmp.persist(&path).map_err(|e| e.error)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(FileStamp(hasher.finalize()))
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub language: String,
    pub theme: i32,
    pub font_size: f32,
    #[serde(default = "default_source_font_size")]
    pub source_font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "default_paragraph_spacing")]
    pub paragraph_spacing: f32,
    #[serde(default = "default_reading_width")]
    pub reading_width: f32,
    pub recent: Vec<PathBuf>,
    pub positions: std::collections::BTreeMap<PathBuf, usize>,
}
fn default_source_font_size() -> f32 {
    14.
}
fn default_reading_width() -> f32 {
    780.
}
fn default_line_height() -> f32 {
    1.5
}
fn default_paragraph_spacing() -> f32 {
    12.
}
impl Preferences {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|v| {
                let saved: serde_json::Value = serde_json::from_slice(&v).ok()?;
                let legacy = saved.get("source_font_size").is_none();
                let mut prefs: Self = serde_json::from_value(saved).ok()?;
                // Migrate only the previous untouched typography preset.
                if legacy
                    && prefs.font_size == 17.
                    && prefs.line_height == 1.65
                    && prefs.paragraph_spacing == 18.
                {
                    prefs.font_size = 15.;
                    prefs.line_height = 1.5;
                    prefs.paragraph_spacing = 12.;
                }
                Some(prefs)
            })
            .unwrap_or(Self {
                font_size: 15.,
                source_font_size: 14.,
                line_height: 1.5,
                paragraph_spacing: 12.,
                reading_width: 780.,
                ..Self::default()
            })
    }
    pub fn store(&self, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        LocalStorage::save(path, &Rope::from_str(&text), None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_typography_migrates_defaults_but_preserves_custom_spacing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        let legacy = r#"{"theme":0,"font_size":17,"line_height":1.65,"paragraph_spacing":18,"recent":[],"positions":{}}"#;
        fs::write(&path, legacy).unwrap();
        let prefs = Preferences::load(&path);
        assert_eq!(
            (
                prefs.font_size,
                prefs.source_font_size,
                prefs.line_height,
                prefs.paragraph_spacing
            ),
            (15., 14., 1.5, 12.)
        );
        fs::write(&path, legacy.replace("1.65", "1.8")).unwrap();
        let prefs = Preferences::load(&path);
        assert_eq!(
            (prefs.font_size, prefs.line_height, prefs.paragraph_spacing),
            (17., 1.8, 18.)
        );
    }
    #[test]
    fn conflict_never_overwrites_external_edit() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("note.md");
        fs::write(&p, "原文\r\n").unwrap();
        let (_, stamp) = LocalStorage::read(&p).unwrap();
        fs::write(&p, "外部修改").unwrap();
        assert!(matches!(
            LocalStorage::save(&p, &Rope::from_str("当前修改"), Some(&stamp)),
            Err(StorageError::Conflict)
        ));
        assert_eq!(fs::read_to_string(&p).unwrap(), "外部修改");
    }
    #[test]
    fn saves_exact_bytes_and_rejects_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("测试.md");
        let text = "\u{feff}# 中文\r\n😀\r\n";
        let stamp = LocalStorage::save(&p, &Rope::from_str(text), None).unwrap();
        assert_eq!(LocalStorage::read(&p).unwrap().0.to_string(), text);
        assert_eq!(LocalStorage::stamp(&p).unwrap(), stamp);
        fs::write(&p, [0xff]).unwrap();
        assert!(matches!(
            LocalStorage::read(&p),
            Err(StorageError::Encoding)
        ));
    }
}
