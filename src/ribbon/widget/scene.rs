//! Immediate-mode scene graph: the ribbon is laid out into a flat list of
//! nodes which is then drawn with iced renderer primitives and used for
//! hit-testing.

use iced::alignment::{Horizontal, Vertical};
use iced::{Color, Rectangle};

use crate::ribbon::image::UiImage;

use super::Target;

/// Fill for a quad: solid color or vertical linear gradient.
#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Color(Color),
    Gradient(Color, Color),
}

impl Fill {
    pub fn top(self) -> Color {
        match self {
            Fill::Color(c) => c,
            Fill::Gradient(top, _) => top,
        }
    }
}

/// Visual command of a scene node.
#[derive(Debug, Clone)]
pub enum Cmd {
    Quad {
        fill: Fill,
        border: Option<(Color, f32)>,
        radius: f32,
    },
    Text {
        text: String,
        size: f32,
        color: Color,
        halign: Horizontal,
        valign: Vertical,
        bold: bool,
    },
    Svg {
        image: UiImage,
        tint: Option<Color>,
        opacity: f32,
    },
    /// Interactive zone that draws nothing.
    Hit,
    /// A keytip badge (drawn on top, not hit-testable).
    KeytipBadge {
        text: String,
        anchor: iced::Point,
    },
}

/// One drawable, hit-testable element.
#[derive(Debug, Clone)]
pub struct Node {
    pub rect: Rectangle,
    pub cmd: Cmd,
    pub target: Option<Target>,
    /// Command whose tooltip should appear when hovered.
    pub tip: Option<u32>,
    /// Keytip text; `strip == true` badges live on the tab strip / chrome.
    pub keytip: Option<(String, bool)>,
}

impl Node {
    pub fn new(rect: Rectangle, cmd: Cmd) -> Self {
        Self {
            rect,
            cmd,
            target: None,
            tip: None,
            keytip: None,
        }
    }

    pub fn target(mut self, t: Target) -> Self {
        self.target = Some(t);
        self
    }
    pub fn tip(mut self, cmd: u32) -> Self {
        self.tip = Some(cmd);
        self
    }
    pub fn keytip(mut self, k: impl Into<String>, strip: bool) -> Self {
        self.keytip = Some((k.into(), strip));
        self
    }
}

/// A flat scene with hit-testing.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub size: iced::Size,
}

impl Scene {
    pub fn new(size: iced::Size) -> Self {
        Self {
            nodes: Vec::new(),
            size,
        }
    }

    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Top-most interactive target at `point`.
    pub fn hit(&self, point: iced::Point) -> Option<&Node> {
        self.nodes
            .iter()
            .rev()
            .find(|n| n.target.is_some() && n.rect.contains(point))
    }

    /// Any node at `point` (for tooltips also on non-interactive parts).
    pub fn hit_any(&self, point: iced::Point) -> Option<&Node> {
        self.nodes
            .iter()
            .rev()
            .find(|n| n.rect.contains(point))
    }

    /// Collect keytips visible in the given stage
    /// (`strip == true` stage shows chrome keytips only).
    pub fn keytips(&self, strip_stage: bool) -> Vec<(String, Target)> {
        self.nodes
            .iter()
            .filter_map(|n| match (&n.keytip, &n.target) {
                (Some((k, strip)), Some(t)) if *strip == strip_stage => {
                    Some((k.clone(), t.clone()))
                }
                _ => None,
            })
            .collect()
    }
}
