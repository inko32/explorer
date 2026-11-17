//! File-operation update logic: clipboard (copy/cut/paste), delete and
//! inline rename. All real filesystem work goes through `Job`s handled
//! on the fs worker thread (src/fs). The `handle` match delegates here.

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
        let cut = mode == ClipMode::Cut;
        // same-folder sources would copy onto themselves and, for cut,
        // delete the original — the same relationship dnd rejects (§5.4)
        let sources: Vec<_> = sources
            .into_iter()
            .filter(|src| crate::dnd::sources_fit(std::slice::from_ref(src), &self.cwd))
            .collect();
        if sources.is_empty() {
            self.status = "The selection is already in this folder".into();
            if cut {
                self.clipboard = None;
            }
            return;
        }
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
