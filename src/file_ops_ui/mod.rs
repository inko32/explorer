//! File Operation UI — the six dialogs from SPEC/File_Operation_UI_Spec.md
//! (D1 progress, D2 conflict confirmation, D3 fine selector, D4 delete
//! confirmation, D5 message box, D6 shortcut wizard).
//!
//! Runs as a SEPARATE explorer process for stability (see the developer
//! note in the spec): the parent starts `explorer --fileop=<base64>` where
//! the payload is line-based `key=value` text (spec: base64 process arg).
//! All operations are sandboxed to `/tmp/fileops_demo` — see engine.rs.
//!
//! Dev-stage entry points:
//!   explorer --fileop=demo               seed /tmp and run a scripted
//!                                        copy-with-conflicts session
//!   explorer --fileop=<base64 payload>   run one real (tmp-sandboxed) op
//! Payload keys: op=copy|move|delete|shortcut, src, dst, paths (a;b),
//!               recycle=1, theme=dark

use iced::widget::Widget as _;
mod b64;
mod d1;
mod d2;
mod d3;
mod d4;
mod d5;
mod d6;
mod engine;
mod format;
mod tokens;
mod widgets;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::keyboard::{key::Named, Event as KeyEvent, Key};
use iced::widget::scrollable;
use iced::{Element, Task, Theme, window};

use engine::{ConflictChoice, FileOp, OpKind, OpState};
use tokens::{Mode, Tokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Event(iced::Event),
    MainOpened(window::Id),
    D3Opened(window::Id),
    WindowClosed(window::Id),
    Noop,
    // D1
    ToggleDetails,
    TogglePause(u64),
    CancelOp(u64),
    PillScroll,
    MainScrolled(f32),
    // D2
    D2Hover {
        op: u64,
        idx: Option<usize>,
    },
    ConflictChoice {
        op: u64,
        choice: ConflictChoice,
    },
    // D3
    D3Toggle {
        op: u64,
        idx: usize,
        side: Side,
    },
    D3Header {
        op: u64,
        side: Side,
    },
    D3SkipSame {
        op: u64,
        checked: bool,
    },
    D3Focus {
        idx: usize,
        side: Side,
    },
    D3Continue(u64),
    D3Cancel(u64),
    // D4/D5
    DeleteYes,
    DeleteNo,
    KeepRunning,
    CancelActions,
    // D6
    WizInput(String),
    WizNext,
    WizBack,
    WizFinish,
    WizCancel,
    /// debug: current window scale factor / physical size (FILEOP_DEBUG=1)
    DebugScale(f32),
    DebugSize(iced::Size),
}

// ---------------------------------------------------------------------------
// payload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum PayloadKind {
    Copy { src: PathBuf, dst: PathBuf },
    Move { src: PathBuf, dst: PathBuf },
    Delete { paths: Vec<PathBuf> },
    Shortcut { target: String },
}

#[derive(Debug, Clone)]
struct Payload {
    kind: PayloadKind,
    recycle: bool,
    mode: Mode,
    demo: bool,
}

fn parse_payload(arg: &str) -> Result<Payload, String> {
    if arg == "demo" {
        return Ok(Payload {
            kind: PayloadKind::Copy {
                src: PathBuf::from(engine::DEMO_ROOT).join("Software"),
                dst: PathBuf::from(engine::DEMO_ROOT).join("{cf3a7d2a-256f-4ef3-b129-6400c44ebd42}"),
            },
            recycle: false,
            mode: Mode::Light,
            demo: true,
        });
    }
    let bytes = b64::decode(arg).ok_or("invalid base64 --fileop payload")?;
    let text = String::from_utf8(bytes).map_err(|_| "payload is not UTF-8")?;
    let mut op = None;
    let mut src = None;
    let mut dst = None;
    let mut paths = Vec::new();
    let mut recycle = false;
    let mut mode = Mode::Light;
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "op" => op = Some(v.trim().to_string()),
            "src" | "target" => src = Some(v.trim().to_string()),
            "dst" => dst = Some(v.trim().to_string()),
            "paths" => {
                paths = v.split(';').filter(|p| !p.is_empty()).map(PathBuf::from).collect();
            }
            "recycle" => recycle = v == "1",
            "theme" => mode = if v == "dark" { Mode::Dark } else { Mode::Light },
            _ => {}
        }
    }
    let kind = match op.as_deref() {
        Some("copy") => PayloadKind::Copy {
            src: PathBuf::from(src.ok_or("payload missing src")?),
            dst: PathBuf::from(dst.ok_or("payload missing dst")?),
        },
        Some("move") => PayloadKind::Move {
            src: PathBuf::from(src.ok_or("payload missing src")?),
            dst: PathBuf::from(dst.ok_or("payload missing dst")?),
        },
        Some("delete") => {
            if paths.is_empty() {
                return Err("payload missing paths".to_string());
            }
            PayloadKind::Delete { paths }
        }
        Some("shortcut") => PayloadKind::Shortcut {
            target: src.unwrap_or_default(),
        },
        other => return Err(format!("unknown op '{}'", other.unwrap_or(""))),
    };
    Ok(Payload {
        kind,
        recycle,
        mode,
        demo: false,
    })
}

// ---------------------------------------------------------------------------
// app state
// ---------------------------------------------------------------------------

enum MsgOrigin {
    Delete,
    CancelActions,
}

pub struct FileOpApp {
    #[allow(dead_code)] // kept for the future shell-side wiring
    payload_kind: PayloadKind,
    recycle: bool,
    t: Tokens,
    main: Option<window::Id>,
    d3_win: Option<window::Id>,
    ops: Vec<FileOp>,
    details: bool,
    scroll_y: f32,
    d2_hover: Option<(u64, usize)>,
    d2_focus: Option<(u64, usize)>,
    d3: Option<d3::D3State>,
    confirm: Option<(d4::DeleteItem, bool)>,
    msgbox: Option<(d5::MsgBox, MsgOrigin)>,
    wizard: Option<d6::Wizard>,
    /// delete paths staged while D4/D5 is up
    pending_delete: Vec<PathBuf>,
    marquee: f32,
    shimmer: f32,
    last_tick: Instant,
    next_id: u64,
    /// demo: second op is staged to start shortly after the first
    demo_pending: Option<PayloadKind>,
    demo_at: Option<Instant>,
    closing: bool,
    exit_at: Option<Instant>,
    last_main_size: iced::Size,
    tick_count: u32,
}

impl FileOpApp {
    fn new(payload: Payload) -> Result<Self, String> {
        if payload.demo {
            engine::seed_demo()?;
        }
        let mut app = Self {
            payload_kind: payload.kind.clone(), // kept for the future shell wiring
            recycle: payload.recycle,
            t: Tokens::new(payload.mode),
            main: None,
            d3_win: None,
            ops: Vec::new(),
            details: false,
            scroll_y: 0.0,
            d2_hover: None,
            d2_focus: None,
            d3: None,
            confirm: None,
            msgbox: None,
            wizard: None,
            pending_delete: Vec::new(),
            marquee: 0.0,
            shimmer: 0.0,
            last_tick: Instant::now(),
            next_id: 1,
            demo_pending: None,
            demo_at: None,
            closing: false,
            exit_at: None,
            last_main_size: iced::Size::ZERO,
            tick_count: 0,
        };
        match payload.kind {
            PayloadKind::Copy { src, dst } => {
                let op = engine::build_copy(app.alloc_id(), src, dst)?;
                app.ops.push(op);
                if payload.demo {
                    // second operation starts a moment later so the
                    // multi-action layout + embedded conflict panel show
                    app.demo_pending = Some(PayloadKind::Copy {
                        src: PathBuf::from(engine::DEMO_ROOT).join("Software/docs"),
                        dst: PathBuf::from(engine::DEMO_ROOT).join("{cf3a7d2a-256f-4ef3-b129-6400c44ebd42}/docs"),
                    });
                    app.demo_at = Some(Instant::now() + Duration::from_millis(2800));
                }
            }
            PayloadKind::Move { src, dst } => {
                let mut op = engine::build_copy(app.alloc_id(), src, dst)?;
                op.kind = OpKind::Move;
                app.ops.push(op);
            }
            PayloadKind::Delete { paths } => {
                app.stage_delete(paths);
            }
            PayloadKind::Shortcut { target } => {
                app.wizard = Some(d6::Wizard::new(target));
            }
        }
        Ok(app)
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// D4 (single) or D5 (multiple) up front, ops start on Yes.
    fn stage_delete(&mut self, paths: Vec<PathBuf>) {
        self.pending_delete = paths.clone();
        if paths.len() == 1
            && let Ok(item) = delete_item(&paths[0])
        {
            self.confirm = Some((item, self.recycle));
            return;
        }
        let mb = d5::MsgBox::delete_multiple(paths.len(), self.recycle);
        self.msgbox = Some((mb, MsgOrigin::Delete));
    }

    fn start_pending_delete(&mut self) {
        let paths = std::mem::take(&mut self.pending_delete);
        if paths.is_empty() {
            return;
        }
        let src_dir = paths[0].parent().map(|p| p.to_path_buf()).unwrap_or_default();
        match engine::build_delete(self.alloc_id(), paths, src_dir) {
            Ok(op) => self.ops.push(op),
            Err(e) => eprintln!("file-ops-ui: {e}"),
        }
    }

    fn main_size(&self) -> iced::Size {
        if self.wizard.is_some() {
            return iced::Size::new(d6::WINDOW_W, d6::WINDOW_H);
        }
        if self.confirm.is_some() {
            return iced::Size::new(d4::WINDOW_W, d4::window_height(4));
        }
        if let Some((mb, _)) = &self.msgbox {
            return iced::Size::new(d5::WINDOW_W, d5::window_height(mb));
        }
        iced::Size::new(d1::WINDOW_W, d1::window_height(self))
    }

    /// Task that resizes the main window when the computed size changed.
    fn sync_main_size(&mut self) -> Task<Message> {
        let size = self.main_size();
        if size == self.last_main_size {
            return Task::none();
        }
        self.last_main_size = size;
        match self.main {
            Some(id) => window::resize(id, size),
            None => Task::none(),
        }
    }

    fn open_d3(&mut self, op_id: u64) -> Task<Message> {
        let Some(op) = self.ops.iter().find(|o| o.id == op_id) else {
            return Task::none();
        };
        self.d3 = Some(d3::D3State {
            op_id,
            src_label: op.src_label.clone(),
            dst_label: op.dst_label.clone(),
            conflicts: op.conflicts.clone(),
            skip_same: false,
            focus: None,
        });
        let (_, open) = window::open(window::Settings {
            size: iced::Size::new(d3::WINDOW_W, d3::WINDOW_H),
            min_size: Some(iced::Size::new(528.0, 360.0)),
            resizable: true,
            ..Default::default()
        });
        open.map(Message::D3Opened)
    }

    fn close_d3(&mut self) -> Task<Message> {
        self.d3 = None;
        match self.d3_win.take() {
            Some(id) => window::close(id),
            None => Task::none(),
        }
    }

    fn exit_soon(&mut self) {
        self.closing = true;
        self.exit_at = Some(Instant::now() + Duration::from_millis(250));
    }

    // ----------------------------------------------------------------- tick

    fn on_tick(&mut self) -> Task<Message> {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_tick).as_secs_f32().min(0.5);
        self.last_tick = now;

        self.marquee = (self.marquee + dt / 2.3) % 1.0;
        self.shimmer = (self.shimmer + dt / 1.6) % 1.2;

        if self.demo_pending.is_some() && self.demo_at.is_some_and(|at| now >= at) {
            self.demo_at = None;
            if let Some(PayloadKind::Copy { src, dst }) = self.demo_pending.take()
                && let Ok(op) = engine::build_copy(self.alloc_id(), src, dst)
            {
                self.ops.push(op);
            }
        }

        for op in &mut self.ops {
            engine::tick(op, dt);
        }
        let before = self.ops.len();
        self.ops.retain(|o| !o.needs_removal());
        let removed = before - self.ops.len();

        if let Some(exit_at) = self.exit_at
            && now >= exit_at
        {
            return iced::exit();
        }

        if self.closing && self.ops.is_empty() {
            return iced::exit();
        }

        // auto-close when the last panel is gone (spec D1.9)
        if self.ops.is_empty()
            && self.confirm.is_none()
            && self.msgbox.is_none()
            && self.wizard.is_none()
            && !self.closing
        {
            // ops never started (bad payload) or all done: leave quickly
            if self.exit_at.is_none() {
                self.exit_at = Some(now + Duration::from_millis(400));
            }
        }

        let _ = removed;

        if std::env::var_os("FILEOP_DEBUG").is_some()
            && self.main.is_some()
            && self.tick_count % 10 == 0
        {
            self.tick_count += 1;
            let id = self.main.unwrap();
            return Task::batch([
                window::scale_factor(id).map(Message::DebugScale),
                window::size(id).map(Message::DebugSize),
            ]);
        }
        self.tick_count += 1;
        self.sync_main_size()
    }

    // --------------------------------------------------------------- update

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DebugScale(scale) => {
                let line12 = crate::text_metrics::line_block(12.0, crate::fonts::primary());
                let line16 = crate::text_metrics::line_block(16.0, crate::fonts::primary());
                eprintln!(
                    "[fileop-geo] scale={scale} logical_h={:.1} line12={line12:.2} line16={line16:.2}",
                    self.last_main_size.height
                );
                Task::none()
            }
            Message::DebugSize(size) => {
                eprintln!("[fileop-geo] physical={size:?}");
                Task::none()
            }
            Message::Tick => self.on_tick(),
            Message::MainOpened(id) => {
                self.main = Some(id);
                self.last_main_size = self.main_size();
                if self.wizard.is_some() {
                    iced::widget::operation::focus(iced::widget::Id::new(d6::INPUT_ID))
                } else {
                    Task::none()
                }
            }
            Message::D3Opened(id) => {
                self.d3_win = Some(id);
                Task::none()
            }
            Message::WindowClosed(id) => {
                if Some(id) == self.d3_win {
                    // closing D3 returns to the D2 prompt (spec D3.3)
                    self.d3_win = None;
                    self.d3 = None;
                    if let Some(op) = self
                        .ops
                        .iter_mut()
                        .find(|o| o.state == OpState::WaitingForD3)
                    {
                        op.state = OpState::NeedsDecision;
                    }
                    Task::none()
                } else if Some(id) == self.main {
                    self.main = None;
                    if let Some(d3) = self.d3_win.take() {
                        window::close(d3)
                    } else {
                        iced::exit()
                    }
                } else {
                    Task::none()
                }
            }
            Message::ToggleDetails => {
                self.details = !self.details;
                self.sync_main_size()
            }
            Message::TogglePause(id) => {
                if let Some(op) = self.ops.iter_mut().find(|o| o.id == id) {
                    match op.state {
                        OpState::Running => op.state = OpState::Paused,
                        OpState::Paused => op.state = OpState::Running,
                        _ => {}
                    }
                }
                Task::none()
            }
            Message::CancelOp(id) => {
                if let Some(op) = self.ops.iter_mut().find(|o| o.id == id) {
                    match op.state {
                        OpState::WaitingForD3 => {
                            op.state = OpState::Cancelling;
                            return self.close_d3();
                        }
                        OpState::Completed | OpState::Cancelling => {}
                        _ => op.state = OpState::Cancelling,
                    }
                }
                Task::none()
            }
            Message::ConflictChoice { op, choice } => {
                let mut open_d3 = false;
                if let Some(o) = self.ops.iter_mut().find(|o| o.id == op) {
                    match choice {
                        ConflictChoice::PerFile => {
                            o.state = OpState::WaitingForD3;
                            open_d3 = true;
                        }
                        other => {
                            o.resolution = Some(other);
                            o.state = OpState::Running;
                        }
                    }
                }
                if open_d3 {
                    self.open_d3(op)
                } else {
                    Task::none()
                }
            }
            Message::D2Hover { op, idx } => {
                self.d2_hover = idx.map(|i| (op, i));
                Task::none()
            }
            Message::PillScroll => {
                // smooth-scroll to the topmost unresolved panel (spec D1.8)
                let content: f32 = self
                    .ops
                    .iter()
                    .map(|o| d1::panel_height(o, self.details) + 1.0)
                    .sum();
                let mut y = 0.0f32;
                for op in &self.ops {
                    if op.state == OpState::NeedsDecision {
                        break;
                    }
                    y += d1::panel_height(op, self.details) + 1.0;
                }
                if content > 0.0 {
                    iced::widget::operation::scrollable::snap_to(
                        iced::widget::Id::new(d1::SCROLL_ID),
                        scrollable::RelativeOffset {
                            x: Some(0.0),
                            y: Some((y / content).clamp(0.0, 1.0)),
                        },
                        iced::widget::operation::Animation::Auto,
                    )
                } else {
                    Task::none()
                }
            }
            Message::MainScrolled(y) => {
                self.scroll_y = y;
                Task::none()
            }
            // -- D3 --
            Message::D3Toggle { op, idx, side } => {
                if let Some(st) = self.d3.as_mut().filter(|s| s.op_id == op)
                    && let Some(c) = st.conflicts.get_mut(idx)
                    && !(st.skip_same && c.same_date_size)
                {
                    match side {
                        Side::Left => c.keep_src = !c.keep_src,
                        Side::Right => c.keep_dst = !c.keep_dst,
                    }
                }
                Task::none()
            }
            Message::D3Header { op, side } => {
                if let Some(st) = self.d3.as_mut().filter(|s| s.op_id == op) {
                    let eligible: Vec<usize> = st
                        .conflicts
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| !(st.skip_same && c.same_date_size))
                        .map(|(i, _)| i)
                        .collect();
                    let all_checked = eligible.iter().all(|&i| {
                        let c = &st.conflicts[i];
                        match side {
                            Side::Left => c.keep_src,
                            Side::Right => c.keep_dst,
                        }
                    });
                    for i in eligible {
                        let c = &mut st.conflicts[i];
                        match side {
                            Side::Left => c.keep_src = !all_checked,
                            Side::Right => c.keep_dst = !all_checked,
                        }
                    }
                }
                Task::none()
            }
            Message::D3SkipSame { op, checked } => {
                if let Some(st) = self.d3.as_mut().filter(|s| s.op_id == op) {
                    st.skip_same = checked;
                }
                Task::none()
            }
            Message::D3Focus { idx, side } => {
                if let Some(st) = self.d3.as_mut() {
                    st.focus = Some((idx, side));
                }
                Task::none()
            }
            Message::D3Continue(op) => {
                let mut task = Task::none();
                if let Some(st) = self.d3.take().filter(|s| s.op_id == op) {
                    if let Some(o) = self.ops.iter_mut().find(|o| o.id == op) {
                        engine::apply_per_file(o, st.conflicts);
                    }
                    task = self.close_d3();
                }
                task
            }
            Message::D3Cancel(op) => {
                if let Some(o) = self.ops.iter_mut().find(|o| o.id == op)
                    && o.state == OpState::WaitingForD3
                {
                    o.state = OpState::NeedsDecision;
                }
                self.close_d3()
            }
            // -- D4/D5 --
            Message::DeleteYes => {
                self.confirm = None;
                self.msgbox = None;
                self.start_pending_delete();
                self.sync_main_size()
            }
            Message::DeleteNo => {
                if self.ops.is_empty() {
                    self.exit_soon();
                    Task::none()
                } else {
                    self.confirm = None;
                    self.msgbox = None;
                    self.sync_main_size()
                }
            }
            Message::KeepRunning => {
                self.msgbox = None;
                self.sync_main_size()
            }
            Message::CancelActions => {
                self.msgbox = None;
                for op in &mut self.ops {
                    if !op.is_finished() {
                        op.state = OpState::Cancelling;
                    }
                }
                self.sync_main_size()
            }
            // -- D6 --
            Message::WizInput(s) => {
                if let Some(w) = self.wizard.as_mut() {
                    w.error = None;
                    if w.step == 1 {
                        w.location = s;
                    } else {
                        w.name = s;
                    }
                }
                Task::none()
            }
            Message::WizNext => {
                if let Some(w) = self.wizard.as_mut() {
                    w.next();
                }
                Task::none()
            }
            Message::WizBack => {
                if let Some(w) = self.wizard.as_mut() {
                    w.step = 1;
                    w.error = None;
                }
                Task::none()
            }
            Message::WizFinish => {
                if let Some(w) = self.wizard.as_mut()
                    && w.finish_step()
                {
                    self.wizard = None;
                    self.exit_soon();
                }
                Task::none()
            }
            Message::WizCancel => {
                self.exit_soon();
                Task::none()
            }
            Message::Event(event) => self.on_event(event),
            Message::Noop => Task::none(),
        }
    }

    fn on_event(&mut self, event: iced::Event) -> Task<Message> {
        let iced::Event::Keyboard(KeyEvent::KeyPressed {
            key, modifiers, ..
        }) = event
        else {
            return Task::none();
        };
        // D3 keyboard map (spec D3.4) — applies while the selector is open
        if self.d3.is_some() {
            return self.d3_key(key);
        }
        match key {
            Key::Named(Named::Escape) => {
                if self.wizard.is_some() {
                    self.wizard = None;
                    self.exit_soon();
                } else if self.confirm.is_some() {
                    self.exit_soon();
                } else if self.msgbox.is_some() {
                    self.msgbox = None;
                    if self.ops.is_empty() {
                        self.exit_soon();
                    }
                } else if !self.ops.is_empty() {
                    // Esc with running ops -> Cancel Actions (spec D1.9)
                    let n = self
                        .ops
                        .iter()
                        .filter(|o| !o.is_finished())
                        .count();
                    let mb = d5::MsgBox::cancel_actions(n.max(1));
                    self.msgbox = Some((mb, MsgOrigin::CancelActions));
                    return self.sync_main_size();
                }
                Task::none()
            }
            Key::Named(Named::Enter) => {
                if self.wizard.is_some() {
                    if self.wizard.as_ref().is_some_and(|w| w.step == 1) {
                        self.update(Message::WizNext)
                    } else {
                        self.update(Message::WizFinish)
                    }
                } else if self.confirm.is_some()
                    || self
                        .msgbox
                        .as_ref()
                        .is_some_and(|(_, o)| matches!(o, MsgOrigin::Delete))
                {
                    // Enter activates the default (destructive) button in
                    // the delete dialogs (spec D4.3 / D5.3)
                    self.update(Message::DeleteYes)
                } else {
                    Task::none()
                }
            }
            Key::Named(Named::ArrowLeft) if modifiers.alt() => {
                if self.wizard.as_ref().is_some_and(|w| w.step == 2) {
                    self.update(Message::WizBack)
                } else {
                    Task::none()
                }
            }
            Key::Character(c) if modifiers.alt() => match c.as_str() {
                "d" | "D" => self.update(Message::ToggleDetails),
                "i" | "I" => self.update(Message::PillScroll),
                _ => Task::none(),
            },
            // D2 direct keys (spec D2.4): R / S / L
            Key::Character(c) => {
                let upper = c.as_str().to_uppercase();
                let any_decision = self.ops.iter().any(|o| o.state == OpState::NeedsDecision);
                if any_decision {
                    match upper.as_str() {
                        "R" => {
                            return self.update(Message::ConflictChoice {
                                op: self.ops[0].id,
                                choice: ConflictChoice::ReplaceAll,
                            })
                        }
                        "S" => {
                            return self.update(Message::ConflictChoice {
                                op: self.ops[0].id,
                                choice: ConflictChoice::SkipAll,
                            })
                        }
                        "L" => {
                            return self.update(Message::ConflictChoice {
                                op: self.ops[0].id,
                                choice: ConflictChoice::PerFile,
                            })
                        }
                        _ => {}
                    }
                }
                // D4/D5 Y/N
                if self.confirm.is_some() || self.msgbox.is_some() {
                    match upper.as_str() {
                        "Y" => return self.update(Message::DeleteYes),
                        "N" => return self.update(Message::DeleteNo),
                        _ => {}
                    }
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn d3_key(&mut self, key: Key) -> Task<Message> {
        let Some(st) = self.d3.as_mut() else {
            return Task::none();
        };
        let n = st.conflicts.len();
        match key {
            Key::Named(Named::Escape) => {
                let op = st.op_id;
                self.d3 = None;
                if let Some(o) = self.ops.iter_mut().find(|o| o.id == op) {
                    o.state = OpState::NeedsDecision;
                }
                self.close_d3()
            }
            Key::Named(Named::Enter) => {
                let op = st.op_id;
                let conflicts = st.conflicts.clone();
                self.d3 = None;
                if let Some(o) = self.ops.iter_mut().find(|o| o.id == op) {
                    engine::apply_per_file(o, conflicts);
                }
                self.close_d3()
            }
            Key::Named(Named::ArrowUp) | Key::Named(Named::ArrowDown) => {
                let delta = if key == Key::Named(Named::ArrowUp) {
                    n.saturating_sub(1)
                } else {
                    1
                };
                let (i, focus_side) = st.focus.unwrap_or((0, Side::Left));
                let ni = if delta == n.saturating_sub(1) {
                    i.saturating_sub(1)
                } else {
                    (i + 1).min(n.saturating_sub(1))
                };
                st.focus = Some((ni, focus_side));
                Task::none()
            }
            Key::Named(Named::ArrowLeft) | Key::Named(Named::ArrowRight) => {
                let (i, _) = st.focus.unwrap_or((0, Side::Left));
                let side = if key == Key::Named(Named::ArrowLeft) {
                    Side::Left
                } else {
                    Side::Right
                };
                st.focus = Some((i, side));
                Task::none()
            }
            Key::Named(Named::Space) => {
                if let Some((i, side)) = st.focus
                    && let Some(c) = st.conflicts.get_mut(i)
                    && !(st.skip_same && c.same_date_size)
                {
                    match side {
                        Side::Left => c.keep_src = !c.keep_src,
                        Side::Right => c.keep_dst = !c.keep_dst,
                    }
                }
                Task::none()
            }
            Key::Named(Named::Home) => {
                st.focus = Some((0, st.focus.map(|(_, s)| s).unwrap_or(Side::Left)));
                Task::none()
            }
            Key::Named(Named::End) => {
                st.focus = Some((
                    n.saturating_sub(1),
                    st.focus.map(|(_, s)| s).unwrap_or(Side::Left),
                ));
                Task::none()
            }
            _ => Task::none(),
        }
    }

    // ----------------------------------------------------------------- view

    fn view<'a>(&'a self, id: window::Id) -> Element<'a, Message> {
        let el = if Some(id) == self.d3_win {
            match &self.d3 {
                Some(st) => d3::view(st, self.t),
                None => iced::widget::Space::new().width(0.0).height(0.0).boxed(),
            }
        } else if let Some(w) = &self.wizard {
            container_styled(d6::view(w, self.t), self.t)
        } else if let Some((item, recycle)) = &self.confirm {
            container_styled(d4::view(item, *recycle, self.t), self.t)
        } else if let Some((mb, _)) = &self.msgbox {
            container_styled(d5::view(mb, self.t), self.t)
        } else {
            container_styled(d1::view(self, self.t), self.t)
        };
        // debug: FILEOP_EXPLAIN=1 draws the layout graph over the content
        if std::env::var_os("FILEOP_EXPLAIN").is_some() {
            use iced::widget::Widget as _;
            el.explain(iced::Color::from_rgb(0.9, 0.1, 0.1)).boxed()
        } else {
            el
        }
    }

    fn title(&self, id: window::Id) -> String {
        if Some(id) == self.d3_win {
            let n = self.d3.as_ref().map(|s| s.conflicts.len()).unwrap_or(0);
            return format!("{n} Total File Conflicts");
        }
        if self.wizard.is_some() {
            return "Create Shortcut".to_string();
        }
        if let Some((item, _)) = &self.confirm {
            return (if item.is_dir { "Delete Folder" } else { "Delete File" }).to_string();
        }
        if let Some((mb, _)) = &self.msgbox {
            return mb.title.clone();
        }
        d1::title(self)
    }
}

fn container_styled(el: Element<Message>, t: Tokens) -> Element<Message> {
    iced::widget::container(el)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(t.surface_content.into()),
            ..Default::default()
        })
        .boxed()
}

/// D4 preview metadata for one path.
fn delete_item(path: &PathBuf) -> Result<d4::DeleteItem, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let is_dir = meta.is_dir();
    let type_label = if is_dir {
        "File folder"
    } else {
        match name.rsplit_once('.').map(|(_, e)| e.to_lowercase()) {
            Some(e) if e == "zip" || e == "7z" => "Archive",
            Some(e) if e == "dll" || e == "exe" => "Application extension",
            Some(e) if e == "txt" || e == "md" => "Text document",
            Some(_) => "File",
            None => "File",
        }
    };
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| {
            let secs = t
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            crate::model::format_date(secs)
        })
        .unwrap_or_default();
    let size = if is_dir {
        // "Contains: N files, M folders" is Proposed; use the aggregate
        dir_size(path)
    } else {
        meta.len()
    };
    Ok(d4::DeleteItem {
        name,
        is_dir,
        type_label: type_label.to_string(),
        size,
        modified,
    })
}

fn dir_size(path: &std::path::Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| dir_size(&e.path()))
                .sum()
        })
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// The daemon view callback as a fn item: ViewFn is HRTB over the state
/// borrow, which closures can't express in this position.
fn view_adapter<'a>(app: &'a FileOpApp, id: window::Id) -> Element<'a, Message> {
    app.view(id)
}

fn tick_stream() -> impl iced::futures::Stream<Item = ()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || loop {
        if tx.send(()).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    });
    iced::futures::stream::unfold(rx, |rx| async move { rx.recv().ok().map(|_| ((), rx)) })
}

pub fn run(arg: &str) -> iced::Result {
    let payload = match parse_payload(arg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("file-ops-ui: {e}");
            std::process::exit(2);
        }
    };
    let app = match FileOpApp::new(payload) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("file-ops-ui: {e}");
            std::process::exit(1);
        }
    };
    let mode = app.t.mode;
    let size = app.main_size();
    let dark = mode == Mode::Dark;

    let boot_app = std::sync::Arc::new(std::sync::Mutex::new(Some(app)));
    iced::daemon(
        move || {
            let app = boot_app
                .lock()
                .expect("file-ops-ui boot lock")
                .take()
                .expect("file-ops-ui booted twice");
            let (_, open) = window::open(window::Settings {
                size,
                resizable: false,
                ..Default::default()
            });
            (app, open.map(Message::MainOpened))
        },
        |app: &mut FileOpApp, msg| app.update(msg),
        view_adapter,
    )
    .font(crate::fonts::default_font())
    .title(|app: &FileOpApp, id| app.title(id))
    .theme(move |_app: &FileOpApp, _id| Some(if dark { Theme::Dark } else { Theme::Light }))
    .subscription(|_app: &FileOpApp| {
        iced::Subscription::batch([
            iced::Subscription::run(tick_stream).map(|()| Message::Tick),
            iced::event::listen().map(Message::Event),
            window::close_events().map(Message::WindowClosed),
        ])
    })
    .run()
}

