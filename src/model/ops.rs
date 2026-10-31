//! File-operation update logic: clipboard (copy/cut/paste), delete,
//! inline rename and the recursive copy helper used by the ribbon
//! "Copy to" command. The `handle` match delegates here.

use std::path::Path;

use crate::fs::Job;

use super::explorer::Explorer;
use super::message::ClipMode;

impl Explorer {
    pub(crate) fn copy_or_cut(&mut self, mode: ClipMode) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            let n = paths.len();
            self.clipboard = Some((mode, paths));
            self.status = match mode {
                ClipMode::Copy => format!("Copied {n} item{}", if n == 1 { "" } else { "s" }),
                ClipMode::Cut => format!("Cut {n} item{}", if n == 1 { "" } else { "s" }),
            };
        }
    }

    pub(crate) fn paste(&mut self) {
        let Some((mode, sources)) = self.clipboard.clone() else {
            return;
        };
        if sources.is_empty() {
            return;
        }
        let cut = mode == ClipMode::Cut;
        self.sched.send(Job::Copy {
            sources,
            dest_dir: self.cwd.clone(),
            cut,
        });
        self.status = "Copying…".into();
        if cut {
            self.clipboard = None;
        }
    }

    pub(crate) fn delete_selection(&mut self) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }
        self.sched.send(Job::Trash(paths));
    }

    pub fn start_rename(&mut self) {
        if let Some(&i) = self.selected.iter().next()
            && i < self.entries.len()
        {
            let name = self.entries[i].name.clone();
            self.renaming = Some((i, name));
        }
    }

    pub(crate) fn commit_rename(&mut self, new_name: String) {
        let name = new_name.trim().to_string();
        if let Some((i, _)) = self.renaming.take()
            && !name.is_empty()
            && i < self.entries.len()
            && name != self.entries[i].name
        {
            let from = self.entries[i].path.clone();
            let to = from.with_file_name(&name);
            self.sched.send(Job::Rename(from, to));
        }
    }
}

/// Recursive copy (files + dirs); true on success.
pub(crate) fn copy_rec(src: &Path, dst: &Path) -> bool {
    if src.is_dir() {
        if std::fs::create_dir_all(dst).is_err() {
            return false;
        }
        let Ok(rd) = std::fs::read_dir(src) else {
            return false;
        };
        for e in rd.flatten() {
            if !copy_rec(&e.path(), &dst.join(e.file_name())) {
                return false;
            }
        }
        true
    } else {
        std::fs::copy(src, dst).is_ok()
    }
}
