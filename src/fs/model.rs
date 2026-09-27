//! Plain-data filesystem model. No egui, no GIO types leak past this file.

use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryKind {
    Dir,
    File,
    Other,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    /// Last modification time as unix seconds (None if unknown).
    pub modified: Option<i64>,
    pub is_hidden: bool,
    pub is_symlink: bool,
    /// Symlink target as display text (when cheaply known).
    pub symlink_target: Option<String>,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Dir
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Volume {
    pub name: String,
    pub mount: PathBuf,
    pub fs_type: Option<String>,
}

#[derive(Debug)]
pub struct FsError(pub String);

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<std::io::Error> for FsError {
    fn from(e: std::io::Error) -> Self {
        FsError(e.to_string())
    }
}

impl From<glib::Error> for FsError {
    fn from(e: glib::Error) -> Self {
        FsError(e.message().to_string())
    }
}
