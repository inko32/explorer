//! Ribbon commanding update logic: the context the ribbon evaluates
//! against, custom shell commands (`{cwd}`/`{selection}` placeholders)
//! and the framework event round-trip. The `handle` match delegates
//! here.

use iced::Task;

use crate::fs;

use super::explorer::Explorer;
use super::message::Message;

impl Explorer {
    /// IUICommandHandler::Execute round-trip: the handler queued
    /// explorer messages; run them through the MVU loop.
    pub(crate) fn on_ribbon(&mut self, ev: crate::ribbon::RibbonEvent) -> Task<Message> {
        self.ribbon.dispatch(&ev);
        let msgs = self.ribbon.take_messages();
        let mut tasks = Vec::new();
        for m in msgs {
            tasks.push(self.update(m));
        }
        Task::batch(tasks)
    }

    /// Run a shell script via `sh -c` (`{cwd}`/`{selection}`
    /// placeholders expanded first).
    pub(crate) fn run_shell(&mut self, script: String) -> Task<Message> {
        let expanded = self.expand_placeholders(&script);
        match std::process::Command::new("sh")
            .arg("-c")
            .arg(expanded)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {}
            Err(e) => self.status = format!("shell: {e}"),
        }
        Task::none()
    }

    /// The context the ribbon commanding layer evaluates against:
    /// contextual-tab availability + enablement rules.
    pub fn context(&self) -> crate::ribbon::Context {
        let sel_has = |f: &dyn Fn(&fs::Entry) -> bool| {
            self.selected
                .iter()
                .filter_map(|&i| self.entries.get(i))
                .any(f)
        };
        let has_image = sel_has(&|e: &fs::Entry| fs::is_image_name(&e.name));
        crate::ribbon::Context {
            is_drive: self.cwd.parent().is_none()
                || self.volumes.iter().any(|v| v.mount == self.cwd),
            inside_archive: fs::archive::resolve(&self.cwd).is_some(),
            has_archive: sel_has(&|e: &fs::Entry| fs::archive::is_archive_path(&e.path)),
            has_image,
            has_selection: !self.selected.is_empty(),
            searching: self.search_active || self.search_query.is_some(),
        }
    }

    pub(crate) fn expand_placeholders(&self, script: &str) -> String {
        fn q(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
        let selection = self
            .selected
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .map(|e| q(&e.path.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ");
        script
            .replace("{cwd}", &q(&self.cwd.to_string_lossy()))
            .replace("{selection}", &selection)
            .replace("{selection_count}", &self.selected.len().to_string())
    }

    // ---- context menus (SPEC/MENU_SPEC.MD) ----
}
