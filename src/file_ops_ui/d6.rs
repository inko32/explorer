//! D6 — Create Shortcut Wizard: two steps (item location, then name)
//! per spec D6.

use iced::widget::{container, mouse_area, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};

#[cfg(unix)]
use super::engine;
use super::tokens::Tokens;
use super::widgets;
use super::Message;

use iced::widget::Widget as _;
pub const WINDOW_W: f32 = 614.0;
pub const WINDOW_H: f32 = 454.0;

pub const INPUT_ID: &str = "fileop-wizard-input";

#[derive(Debug, Clone)]
pub struct Wizard {
    pub step: u8,
    pub location: String,
    pub name: String,
    pub error: Option<String>,
}

impl Wizard {
    pub fn new(location: String) -> Self {
        Self {
            step: 1,
            location,
            name: String::new(),
            error: None,
        }
    }

    /// Default shortcut name: last segment, extension stripped (spec D6.3).
    fn default_name(location: &str) -> String {
        let last = location
            .trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("");
        let stem = last.split_once('.').map(|(s, _)| s).unwrap_or(last);
        stem.to_string()
    }

    /// Step-1 Next validation (spec D6.3).
    fn validate_location(&mut self) -> bool {
        let loc = self.location.trim();
        if loc.is_empty() {
            return false;
        }
        let path = std::path::Path::new(loc);
        if path.exists() {
            self.step = 2;
            self.error = None;
            if self.name.is_empty() {
                self.name = Self::default_name(loc);
            }
            true
        } else {
            self.error = Some(format!(
                "Windows can't find '{loc}'. Check the spelling and try again."
            ));
            false
        }
    }

    fn name_valid(&self) -> bool {
        const INVALID: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        !self.name.is_empty() && !self.name.chars().any(|c| INVALID.contains(&c))
    }

    /// Create the shortcut (a symlink inside the demo root at this stage).
    fn finish(&self) {
        let target = self.location.trim();
        let name = self.name.trim();
        if target.is_empty() || name.is_empty() {
            return;
        }
        // symlink creation needs a privilege on Windows we don't
        // require; unsupported there (same rule as fs::link_one)
        #[cfg(unix)]
        {
            let link = std::path::Path::new(engine::DEMO_ROOT).join(name);
            let _ = std::os::unix::fs::symlink(target, &link);
        }
        #[cfg(not(unix))]
        let _ = target;
    }

    pub fn next(&mut self) {
        if self.step == 1 {
            self.validate_location();
        }
    }

    pub fn finish_step(&mut self) -> bool {
        if self.step == 2 && self.name_valid() {
            self.finish();
            return true;
        }
        if !self.name_valid() {
            self.error = Some(
                "A file name can't contain any of the following characters: \\ / : * ? \" < > |"
                    .to_string(),
            );
        }
        false
    }
}

pub fn view<'a>(wiz: &'a Wizard, t: Tokens) -> Element<'a, Message> {
    let back = text("←").size(14).color(if wiz.step == 2 {
        t.chevron
    } else {
        t.text_disabled
    });
    let back: Element<'a, Message> = if wiz.step == 2 {
        mouse_area(back).on_press(Message::WizBack).boxed()
    } else {
        back.boxed()
    };

    let header = row![
        container(back).padding(Padding {
            left: 9.0,
            right: 9.0,
            top: 6.0,
            bottom: 6.0,
        }),
        text("↗").size(13).color(t.accent_fill),
        text("Create Shortcut").size(12).color(t.text_wizard_header),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let instruction = if wiz.step == 1 {
        "What item would you like to create a shortcut for?"
    } else {
        "What would you like to name the shortcut?"
    };

    let body: Element<'a, Message> = if wiz.step == 1 {
        let description = text(
            "This wizard helps you to create shortcuts to local or network programs, files, folders, computers, or\nInternet addresses.",
        )
        .size(12)
        .color(t.text_secondary);
        let input = text_input("Type the location of the item:", &wiz.location)
            .on_input(Message::WizInput)
            .width(Length::Fixed(408.0))
            .padding(Padding {
                left: 4.0,
                right: 4.0,
                top: 4.0,
                bottom: 4.0,
            })
            .size(12);
        let browse = widgets::push_btn("Browse...", 86.0, false, false, Message::Noop, t);
        let error_line: Element<'a, Message> = match &wiz.error {
            Some(e) => row![
                text("✕").size(12).color(t.error_fill),
                text(e.clone()).size(12).color(t.error_fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .boxed(),
            None => Space::new().width(0.0).height(0.0).boxed(),
        };
        container(
            iced::widget::column![
                text(instruction).size(16).color(t.accent_text),
                description,
                Space::new().width(1.0).height( 6.0),
                row![
                    container(input).height(Length::Fixed(26.0)),
                    browse,
                ]
                .spacing(9)
                .align_y(Alignment::Center),
                Space::new().width(1.0).height( 4.0),
                error_line,
                Space::new().width(1.0).height( 4.0),
                text("Click Next to continue.").size(12).color(t.text_secondary),
            ]
            .spacing(8),
        )
        .padding(Padding {
            left: 39.0,
            top: 39.0,
            ..Default::default()
        })
        .boxed()
    } else {
        let input = text_input("Type a name for this shortcut:", &wiz.name)
            .on_input(Message::WizInput)
            .width(Length::Fixed(327.0))
            .padding(Padding {
                left: 4.0,
                right: 4.0,
                top: 4.0,
                bottom: 4.0,
            })
            .size(12);
        let error_line: Element<'a, Message> = match &wiz.error {
            Some(e) => row![
                text("✕").size(12).color(t.error_fill),
                text(e.clone()).size(12).color(t.error_fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .boxed(),
            None => Space::new().width(0.0).height(0.0).boxed(),
        };
        container(
            iced::widget::column![
                text(instruction).size(16).color(t.accent_text),
                Space::new().width(1.0).height( 26.0),
                text("Type a name for this shortcut:").size(12).color(t.text_primary),
                container(input).height(Length::Fixed(26.0)),
                error_line,
                text("Click Finish to create the shortcut.")
                    .size(12)
                    .color(t.text_secondary),
            ]
            .spacing(8),
        )
        .padding(Padding {
            left: 39.0,
            top: 39.0,
            ..Default::default()
        })
        .boxed()
    };

    // footer: divider + Next/Finish + Cancel
    let primary_label = if wiz.step == 1 { "Next" } else { "Finish" };
    let can_next = if wiz.step == 1 {
        !wiz.location.trim().is_empty()
    } else {
        wiz.name_valid()
    };
    let primary = widgets::push_btn(
        primary_label,
        70.0,
        true,
        can_next,
        if wiz.step == 1 {
            Message::WizNext
        } else {
            Message::WizFinish
        },
        t,
    );
    let cancel = widgets::push_btn("Cancel", 70.0, false, true, Message::WizCancel, t);

    let footer = container(
        row![Space::new().width(Length::Fill).height( 1.0), primary, cancel]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Length::Fixed(WINDOW_W))
    .height(Length::Fixed(41.0))
    .padding(Padding {
        left: 12.0,
        right: 12.0,
        ..Default::default()
    })
    .style(move |_| container::Style {
        background: Some(t.surface_dialog.into()),
        ..Default::default()
    });

    iced::widget::column![
        container(header).padding(Padding {
            left: 0.0,
            top: 8.0,
            ..Default::default()
        }),
        body,
        super::d1::divider(t),
        footer
    ]
    .width(Length::Fixed(WINDOW_W))
    .boxed()
}
