//! Cross-platform drag-and-drop core — SPEC/DRAGGING.md.
//!
//! Phase 1 (internal model, registry, policy, normalized events) and
//! Phase 2 (internal file dragging) live here. Everything in this file
//! is platform-independent; no Wayland/X11/Win32 types (§2.2).
//!
//! Platform status (§7.2, verified against iced @ cd04b74 / the pinned
//! winit fork): iced exposes no drag-OUT payload API (`window::drag`
//! only moves the window), and drag-IN arrives as window events only
//! where winit implements file drops (X11 / Windows / macOS — the fork
//! has no Wayland data-device yet). See [`capabilities`].

use std::path::{Path, PathBuf};

use iced::Element;
use iced::Widget as _;

use crate::model::Message;

/// Logical pixels of pointer travel before a row press becomes a drag.
pub const DRAG_THRESHOLD: f32 = 5.0;

// ---------------- §3.1 payload ----------------

/// What a drag offers. Represents offered data only — never implies a
/// file operation already happened (§3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum DragPayload {
    Files(Vec<DraggedItem>),
    UriList(Vec<String>),
    Text(String),
    Custom { mime_type: String, data: Vec<u8> },
}

impl DragPayload {
    /// Item count (0 for non-file payloads).
    pub fn len(&self) -> usize {
        match self {
            DragPayload::Files(items) => items.len(),
            DragPayload::UriList(uris) => uris.len(),
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DraggedItem {
    /// `file://` URI for local files — URIs are never plain paths (§3.1).
    pub uri: String,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// `file://` URI for a local path (percent-encoded, §13.2 non-ASCII).
pub fn file_uri(path: &Path) -> String {
    let mut s = String::from("file://");
    for &b in path.to_string_lossy().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                s.push(b as char)
            }
            _ => s.push_str(&format!("%{:02X}", b)),
        }
    }
    s
}

/// Local path behind a `file://` URI (None for any other scheme).
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(&bytes[i + 1]), hex(&bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

fn hex(b: &u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------- §3.2 / §3.3 source + session ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragSource {
    FileView,
    NavigationSidebar,
    AddressBar,
    External,
}

#[derive(Debug, Clone)]
pub struct DragSession {
    pub payload: DragPayload,
    pub source: DragSource,
    /// Right-button drag: the drop never commits directly — it opens
    /// the action-choice menu (like Shift+drag).
    pub right: bool,
    /// URI of the item under the cursor when the drag began — the
    /// "top" item of the drag-image icon stack; None when unknown.
    pub top: Option<String>,
}

// ---------------- §3.4 actions ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropAction {
    Copy,
    Move,
    Link,
    Open,
    Run,
    Import,
    /// Never results in a file operation (§3.4).
    Reject,
}

// ---------------- §4 drop targets ----------------

#[derive(Debug, Clone, PartialEq)]
pub enum DropTarget {
    Directory(PathBuf),
    NavigationDirectory(PathBuf),
    Breadcrumb(PathBuf),
    File(PathBuf),
    EmptyDirectory(PathBuf),
    Executable(PathBuf),
    Tab(PathBuf),
    Favorites,
}

impl DropTarget {
    /// The filesystem location a committed drop acts on.
    pub fn path(&self) -> Option<&Path> {
        match self {
            DropTarget::Directory(p)
            | DropTarget::NavigationDirectory(p)
            | DropTarget::Breadcrumb(p)
            | DropTarget::File(p)
            | DropTarget::EmptyDirectory(p)
            | DropTarget::Executable(p)
            | DropTarget::Tab(p) => Some(p),
            DropTarget::Favorites => None,
        }
    }

    /// §6 registry priority (higher wins).
    pub fn priority(&self) -> u8 {
        match self {
            // explicit interactive zones come first
            DropTarget::Executable(_) | DropTarget::Favorites => 4,
            // then the item under the pointer
            DropTarget::Directory(_) | DropTarget::File(_) => 3,
            // then crumb / navigation rows
            DropTarget::Breadcrumb(_) | DropTarget::NavigationDirectory(_) | DropTarget::Tab(_) => 2,
            // then the empty area of the active directory
            DropTarget::EmptyDirectory(_) => 1,
        }
    }
}

// ---------------- §4.1 target registry ----------------

/// Drop-target registry (§4.1).
///
/// The single-window iced UI resolves the target under the pointer from
/// widget enter/exit events — hover hit-testing follows the rendered
/// widget tree, so the §6 nesting priority holds by construction and no
/// per-frame geometry is ever read. The `register_bounds`/`hit_test`
/// side is the registration API a native platform adapter (Phase 3+)
/// drives with compositor geometry; it is not used by the internal UI.
#[derive(Default)]
pub struct TargetRegistry {
    /// enter/exit-balanced stack of live targets; the top is the
    /// innermost (highest-priority) target under the pointer.
    stack: Vec<DropTarget>,
    /// bounds-based registrations (native adapters).
    bounds: Vec<(DropTarget, iced::Rectangle)>,
}

impl TargetRegistry {
    pub fn enter(&mut self, target: DropTarget) {
        // re-entering the current target is a no-op (avoids unbalanced
        // duplicates when the pointer wobbles on a boundary)
        if self.stack.last() != Some(&target) {
            self.stack.retain(|t| t != &target);
            self.stack.push(target);
        }
    }

    pub fn leave(&mut self, target: &DropTarget) {
        if let Some(pos) = self.stack.iter().rposition(|t| t == target) {
            self.stack.remove(pos);
        }
    }

    /// Highest-priority live target under the pointer, if any.
    pub fn current(&self) -> Option<&DropTarget> {
        self.stack.last()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    // Bounds-based registration for the native platform adapters
    // (Phase 3+); the internal iced UI drives `enter`/`leave` instead.

    pub fn register_bounds(&mut self, target: DropTarget, rect: iced::Rectangle) {
        self.bounds.push((target, rect));
    }

    pub fn update_bounds(&mut self, target: &DropTarget, rect: iced::Rectangle) {
        for (t, r) in &mut self.bounds {
            if t == target {
                *r = rect;
                return;
            }
        }
    }

    pub fn unregister(&mut self, target: &DropTarget) {
        self.bounds.retain(|(t, _)| t != target);
    }

    /// Highest-priority registered target containing `point`
    /// (deterministic: priority first, then registration order).
    pub fn hit_test(&self, point: iced::Point) -> Option<DropTarget> {
        self.bounds
            .iter()
            .filter(|(_, r)| r.contains(point))
            .max_by_key(|(t, _)| t.priority())
            .map(|(t, _)| t.clone())
    }
}

/// The floating caption shown beside the cursor while dragging (§5.1
/// feedback rendered as a drag-image caption).
#[derive(Debug, Clone, PartialEq)]
pub enum DragCaption {
    /// Red banned glyph: no action is possible here.
    Banned,
    /// Hovering the dragged items themselves (or their current folder):
    /// nothing is shown — the drop would be a no-op, not a rejection.
    Hidden,
    /// "…": the action is undecided (Shift+drag / right-button drag) —
    /// releasing opens a menu to pick the behavior.
    Undecided,
    /// A definite action; `label` is the full caption text
    /// ("Move to …", "Copy to …", "Link here", "Run with …").
    Action {
        action: DropAction,
        label: String,
    },
}

// ---------------- §4.2 policy ----------------

/// §5.4 source/target relationship (the path-only part of
/// `validate_commit`): a drop into the item itself, into its own
/// descendant, or back into its current parent folder can never
/// complete — the hover caption shows the banned glyph for these.
fn sources_fit(sources: &[PathBuf], dest: &Path) -> bool {
    sources
        .iter()
        .all(|src| dest != src && !dest.starts_with(src) && src.parent() != Some(dest))
}

/// Permitted actions for a payload/target pair (§4.2 defaults —
/// conservative: nothing destructive is ever inferred from proximity).
pub fn permitted_actions(payload: &DragPayload, target: &DropTarget) -> Vec<DropAction> {
    match payload {
        DragPayload::Files(items) => {
            let sources: Vec<PathBuf> = items
                .iter()
                .filter_map(|i| uri_to_path(&i.uri))
                .collect();
            // only entirely-local drags can be file-operated; foreign
            // URIs would need an importer (§9) that does not exist yet
            let local = !items.is_empty() && sources.len() == items.len();
            match target {
                DropTarget::Directory(d)
                | DropTarget::NavigationDirectory(d)
                | DropTarget::Breadcrumb(d)
                | DropTarget::EmptyDirectory(d)
                | DropTarget::Tab(d) => {
                    // drops commit would reject must not be advertised:
                    // the self/parent relationship (pure) and a live
                    // writability probe (read-only / permission denied)
                    if local && sources_fit(&sources, d) && crate::fs::writable(d) {
                        vec![DropAction::Copy, DropAction::Move, DropAction::Link]
                    } else {
                        vec![]
                    }
                }
                DropTarget::Executable(_) => vec![DropAction::Run],
                // regular files reject by default; "open / replace" needs
                // an explicit policy + conflict dialog (future)
                DropTarget::File(_) => vec![],
                DropTarget::Favorites => vec![DropAction::Copy],
            }
        }
        // text / URI-list / custom drops have no importer yet (§9)
        _ => vec![],
    }
}

/// §3.4: map the held modifiers to the offered action. The default is
/// Move — dragging within the manager relocates items (Windows Explorer
/// semantics); Ctrl overrides to Copy, Shift forces Move, Ctrl+Shift/Alt
/// creates a link. (`Session` intercepts Shift earlier: a Shift+drag is
/// action-ambiguous and its drop opens the choice menu, so the shift
/// branch only serves callers without that convention.)
pub fn resolve_action(
    ctrl: bool,
    shift: bool,
    alt: bool,
    permitted: &[DropAction],
) -> DropAction {
    let preferred = if (ctrl && shift) || alt {
        DropAction::Link
    } else if shift {
        DropAction::Move
    } else if ctrl {
        DropAction::Copy
    } else {
        DropAction::Move
    };
    if permitted.contains(&preferred) {
        preferred
    } else {
        DropAction::Reject
    }
}

/// §5.4 / §12 revalidation immediately before committing: the target
/// must still exist, still be eligible, and the source/target
/// relationship must not make the operation destructive or meaningless.
pub fn validate_commit(
    sources: &[PathBuf],
    target: &DropTarget,
    action: DropAction,
) -> Result<(), String> {
    use DropAction::*;
    if action == Reject {
        return Err("the drop was rejected".into());
    }
    let Some(dest) = target.path() else {
        return Err("this target cannot accept the drop".into());
    };
    if crate::fs::archive::resolve(dest).is_some() {
        return Err("archives are read-only".into());
    }
    match action {
        Copy | Move | Link => {
            if !dest.is_dir() {
                return Err(format!("{} is not a folder", dest.display()));
            }
            // §4.2: the hover policy probes this too — re-checked here
            // because the destination may have changed mid-drag
            if !crate::fs::writable(dest) {
                return Err(format!(
                    "{} is read-only or permission was denied",
                    dest.display()
                ));
            }
        }
        Run => {
            if !dest.is_file() {
                return Err("not an executable file".into());
            }
        }
        Open | Import | Reject => {
            return Err(format!("{action:?} drops are not implemented"));
        }
    }
    for src in sources {
        // a folder dropped onto itself or into its own descendant
        if dest == src || dest.starts_with(src) {
            return Err("cannot drop an item into itself".into());
        }
        // same-parent drops: Move would copy-rename then delete the
        // source (data loss of the original name), Copy would spawn a
        // pointless "name (2)" — Windows rejects both
        if src.parent() == Some(dest) {
            return Err("the item is already in that folder".into());
        }
    }
    Ok(())
}

// ---------------- §5 event lifecycle ----------------

/// Normalized drag events (§5). The model maps raw input onto these;
/// [`Session::apply`] maintains registry + hover state and returns the
/// status-bar feedback text.
#[derive(Debug, Clone, PartialEq)]
pub enum DragEvent {
    Enter { target: DropTarget },
    Move { target: DropTarget },
    Leave { target: DropTarget },
    Drop { target: DropTarget, action: DropAction },
    Cancel,
}

/// The Drag Session Manager (§2.1): owns the active session, the target
/// registry and the modifier-derived action feedback.
#[derive(Default)]
pub struct Session {
    session: Option<DragSession>,
    registry: TargetRegistry,
    ctrl: bool,
    shift: bool,
    alt: bool,
    /// The caption last computed from (session, target, modifiers) —
    /// rebuilt only on those inputs, so view() reads it for free.
    caption: Option<DragCaption>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, session: DragSession) {
        self.registry.clear();
        self.session = Some(session);
        self.refresh_caption();
    }

    pub fn active(&self) -> bool {
        self.session.is_some()
    }

    pub fn payload(&self) -> Option<&DragPayload> {
        self.session.as_ref().map(|s| &s.payload)
    }

    /// The URI of the item grabbed by the cursor at drag start (the
    /// top of the drag-image icon stack); None when not recorded.
    pub fn top(&self) -> Option<&str> {
        self.session.as_ref()?.top.as_deref()
    }

    /// Take the session at drop/cancel time (clears all drag state).
    pub fn take(&mut self) -> Option<DragSession> {
        self.registry.clear();
        self.caption = None;
        self.session.take()
    }

    pub fn current(&self) -> Option<&DropTarget> {
        self.registry.current()
    }

    /// True while a right-button drag is active (its drop opens the
    /// action-choice menu instead of committing).
    pub fn right(&self) -> bool {
        self.session.as_ref().map(|s| s.right).unwrap_or(false)
    }

    /// The drag is action-ambiguous (Shift+drag / right-button drag):
    /// the caption is "…" and the drop opens the choice menu.
    pub fn ambiguous(&self) -> bool {
        self.session.as_ref().map(|s| s.right).unwrap_or(false) || self.shift
    }

    /// The caption for the cursor overlay (None when no drag is active).
    pub fn caption(&self) -> Option<&DragCaption> {
        self.caption.as_ref()
    }

    pub fn set_modifiers(&mut self, ctrl: bool, shift: bool, alt: bool) {
        self.ctrl = ctrl;
        self.shift = shift;
        self.alt = alt;
        self.refresh_caption();
    }

    /// Apply a normalized event; returns the feedback for the status bar.
    pub fn apply(&mut self, event: DragEvent) -> String {
        match &event {
            DragEvent::Enter { target } | DragEvent::Move { target } => {
                self.registry.enter(target.clone());
            }
            DragEvent::Leave { target } => {
                self.registry.leave(target);
            }
            DragEvent::Drop { .. } | DragEvent::Cancel => {
                self.registry.clear();
            }
        }
        self.refresh_caption();
        self.feedback()
    }

    /// §5.1/§5.3 feedback: what would happen if released right now.
    pub fn feedback(&self) -> String {
        let Some(session) = &self.session else {
            return String::new();
        };
        let n = session.payload.len();
        let Some(target) = self.registry.current() else {
            return format!("Dragging {n} item{}", if n == 1 { "" } else { "s" });
        };
        let permitted = permitted_actions(&session.payload, target);
        if permitted.is_empty() {
            return "Cannot drop here".into();
        }
        if self.ambiguous() {
            return "Release to choose an action".into();
        }
        let action = resolve_action(self.ctrl, self.shift, self.alt, &permitted);
        if action == DropAction::Reject {
            "Cannot drop here".into()
        } else {
            match target.path() {
                Some(p) => format!("{action:?} to {}", p.display()),
                None => format!("{action:?}"),
            }
        }
    }

    /// Rebuild the cached caption from (session, target, modifiers).
    fn refresh_caption(&mut self) {
        self.caption = self.compute_caption();
    }

    fn compute_caption(&self) -> Option<DragCaption> {
        let session = self.session.as_ref()?;
        let Some(target) = self.registry.current() else {
            // no target under the pointer (window chrome): nothing can
            // happen there, same as a rejecting target
            return Some(DragCaption::Banned);
        };
        // hovering the dragged items themselves, their own subtree, or
        // their current parent folder: the drop would be a no-op — no
        // badge at all (a rejection glyph would flicker over every
        // harmless re-hover of the source)
        if let DragPayload::Files(items) = &session.payload {
            let sources: Vec<PathBuf> =
                items.iter().filter_map(|i| uri_to_path(&i.uri)).collect();
            if let Some(dest) = target.path()
                && !sources.is_empty()
                && !sources_fit(&sources, dest)
            {
                return Some(DragCaption::Hidden);
            }
        }
        let permitted = permitted_actions(&session.payload, target);
        if permitted.is_empty() {
            return Some(DragCaption::Banned);
        }
        if self.ambiguous() {
            return Some(DragCaption::Undecided);
        }
        // an executable offers Run and nothing else: no modifier
        // negotiation, the caption is always "Run with"
        let action = if permitted.contains(&DropAction::Run) {
            DropAction::Run
        } else {
            resolve_action(self.ctrl, self.shift, self.alt, &permitted)
        };
        // the caption names the destination's folder, not its full path
        let name = target
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .or_else(|| target.path().map(|p| p.display().to_string()));
        let label = match action {
            DropAction::Move => match &name {
                Some(n) => format!("Move to \"{n}\""),
                None => "Move here".into(),
            },
            DropAction::Copy => match &name {
                Some(n) => format!("Copy to \"{n}\""),
                None => "Copy here".into(),
            },
            DropAction::Link => "Link here".to_string(),
            DropAction::Run => match &name {
                Some(n) => format!("Run with \"{n}\""),
                None => return Some(DragCaption::Banned),
            },
            _ => return Some(DragCaption::Banned),
        };
        Some(DragCaption::Action { action, label })
    }
}

// ---------------- §7 platform adapter ----------------

/// Capabilities of the active platform stack (§7.2 — verified against
/// iced @ cd04b74 and its pinned winit fork).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapabilities {
    /// External drag-in arrives as `window::Event::FileHovered/FileDropped`.
    pub drag_in: bool,
    /// External drag-out payloads (iced has no such API yet).
    pub drag_out: bool,
    pub multiple_files: bool,
    /// Native copy/move/link negotiation (not exposed by iced).
    pub action_negotiation: bool,
}

pub fn capabilities() -> PlatformCapabilities {
    // winit implements file drops on X11/Windows/macOS only; the pinned
    // fork has no Wayland data-device. Wayland-first app: report by the
    // session type.
    #[cfg(target_os = "linux")]
    let drag_in = std::env::var_os("WAYLAND_DISPLAY").is_none();
    #[cfg(not(target_os = "linux"))]
    let drag_in = true;
    PlatformCapabilities {
        drag_in,
        drag_out: false,
        multiple_files: true,
        action_negotiation: false,
    }
}

// ---------------- §6 UI integration ----------------

/// Register `content` as a drop target: publishes Enter/Leave while a
/// drag is active (§6 — hover hit-testing, no geometry bookkeeping).
pub(crate) fn drop_zone<'a>(
    content: Element<'a, Message>,
    target: DropTarget,
) -> Element<'a, Message> {
    iced::widget::mouse_area(content)
        .on_enter(Message::DragTargetEnter(target.clone()))
        .on_exit(Message::DragTargetLeave(target))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_roundtrip() {
        for p in [
            PathBuf::from("/home/user/file.txt"),
            PathBuf::from("/tmp/with space/üñïcode.rs"),
            PathBuf::from("/a%b/100%.png"),
        ] {
            assert_eq!(uri_to_path(&file_uri(&p)).as_deref(), Some(p.as_path()));
        }
        assert_eq!(uri_to_path("https://example.com/x"), None);
    }

    #[test]
    fn policy_directories_accept_files() {
        // a source from a different folder (same-parent/self drops are
        // banned — see policy_bans_self_parent_and_readonly)
        let payload = DragPayload::Files(vec![DraggedItem {
            uri: "file:///data/a".into(),
            kind: ItemKind::File,
        }]);
        for t in [
            DropTarget::Directory("/tmp".into()),
            DropTarget::NavigationDirectory("/tmp".into()),
            DropTarget::Breadcrumb("/tmp".into()),
            DropTarget::EmptyDirectory("/tmp".into()),
        ] {
            let p = permitted_actions(&payload, &t);
            assert!(p.contains(&DropAction::Copy));
            assert!(p.contains(&DropAction::Move));
            assert!(p.contains(&DropAction::Link));
        }
        // regular files reject by default
        assert!(permitted_actions(&payload, &DropTarget::File("/tmp/x".into())).is_empty());
        // foreign (non-file://) items cannot be file-operated
        let foreign = DragPayload::Files(vec![DraggedItem {
            uri: "https://example.com/f".into(),
            kind: ItemKind::Other,
        }]);
        assert!(permitted_actions(&foreign, &DropTarget::Directory("/tmp".into())).is_empty());
    }

    /// Hovering the dragged item itself, its own subtree, its current
    /// parent folder, or a read-only / permission-denied destination
    /// offers nothing — the caption shows the banned glyph.
    #[test]
    fn policy_bans_self_parent_and_readonly() {
        let src = PathBuf::from("/tmp/x_src");
        let payload = DragPayload::Files(vec![DraggedItem {
            uri: file_uri(&src),
            kind: ItemKind::File,
        }]);
        // the dragged item itself, its own descendant, its parent
        assert!(permitted_actions(&payload, &DropTarget::Directory(src.clone())).is_empty());
        assert!(permitted_actions(&payload, &DropTarget::Directory(src.join("child"))).is_empty());
        assert!(permitted_actions(&payload, &DropTarget::Directory(PathBuf::from("/tmp"))).is_empty());
        assert!(
            permitted_actions(&payload, &DropTarget::EmptyDirectory(PathBuf::from("/tmp"))).is_empty()
        );
        // a genuine sibling folder stays droppable
        let tmp = std::env::temp_dir().join("explorer_policy_writable");
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(!permitted_actions(&payload, &DropTarget::Directory(tmp.clone())).is_empty());
        // a read-only destination is banned (chmod has no effect on Windows)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o555)).unwrap();
            assert!(permitted_actions(&payload, &DropTarget::Directory(tmp.clone())).is_empty());
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_action_modifiers() {
        let all = [DropAction::Copy, DropAction::Move, DropAction::Link];
        assert_eq!(resolve_action(false, false, false, &all), DropAction::Move);
        assert_eq!(resolve_action(true, false, false, &all), DropAction::Copy);
        assert_eq!(resolve_action(false, true, false, &all), DropAction::Move);
        assert_eq!(resolve_action(true, true, false, &all), DropAction::Link);
        assert_eq!(resolve_action(false, false, true, &all), DropAction::Link);
        // not permitted -> reject
        assert_eq!(resolve_action(false, true, false, &[DropAction::Copy]), DropAction::Reject);
    }

    #[test]
    fn validate_rejects_self_and_same_parent() {
        let src = PathBuf::from("/data/folder");
        // into itself / its own descendant
        assert!(validate_commit(&[src.clone()], &DropTarget::Directory(src.clone()), DropAction::Copy).is_err());
        assert!(validate_commit(&[src.clone()], &DropTarget::Directory(src.join("child")), DropAction::Copy).is_err());
        // same-parent copy and move are both rejected
        assert!(validate_commit(&[src.clone()], &DropTarget::Directory(PathBuf::from("/data")), DropAction::Copy).is_err());
        assert!(validate_commit(&[src.clone()], &DropTarget::Directory(PathBuf::from("/data")), DropAction::Move).is_err());
        // a genuine cross-folder drop is fine (dest must exist: §5.4
        // revalidates against the live filesystem)
        let tmp = std::env::temp_dir();
        assert!(validate_commit(&[src], &DropTarget::Directory(tmp), DropAction::Copy).is_ok());
        // archives are read-only
        assert!(validate_commit(
            &[PathBuf::from("/data/a.txt")],
            &DropTarget::Directory(PathBuf::from("/data/pics.zip")),
            DropAction::Copy,
        ).is_err());
    }

    #[test]
    fn registry_stack_nesting() {
        let mut r = TargetRegistry::default();
        let empty = DropTarget::EmptyDirectory("/a".into());
        let dir = DropTarget::Directory("/a/sub".into());
        r.enter(empty.clone());
        r.enter(dir.clone());
        assert_eq!(r.current(), Some(&dir));
        r.leave(&dir);
        assert_eq!(r.current(), Some(&empty));
        r.leave(&empty);
        assert_eq!(r.current(), None);
        // leaving an unknown target is a no-op
        r.leave(&dir);
    }

    #[test]
    fn registry_hit_test_priority() {
        let mut r = TargetRegistry::default();
        let empty = DropTarget::EmptyDirectory("/a".into());
        let dir = DropTarget::Directory("/a/sub".into());
        let rect = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(100.0, 100.0));
        r.register_bounds(empty.clone(), rect);
        r.register_bounds(dir.clone(), iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(10.0, 10.0)));
        assert_eq!(r.hit_test(iced::Point::new(5.0, 5.0)), Some(dir.clone()));
        assert_eq!(r.hit_test(iced::Point::new(50.0, 50.0)), Some(empty.clone()));
        r.unregister(&dir);
        assert_eq!(r.hit_test(iced::Point::new(5.0, 5.0)), Some(empty));
    }
}
