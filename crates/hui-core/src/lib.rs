//! Platform independent, lossless document editing. All document offsets are Unicode scalar offsets.
pub mod document;
pub mod search;
pub mod storage;

pub use document::{Document, Edit, Selection, Viewport};
pub use storage::{FileStamp, LocalStorage, StorageError};

pub mod format;
