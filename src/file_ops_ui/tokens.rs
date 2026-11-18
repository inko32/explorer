//! Theme tokens for the file-operation dialogs (spec §2).
//!
//! Only the light and dark token sets are implemented here; the accent
//! ramp is fixed to the default Windows 11 blue (spec §2.2 fallback) and
//! high contrast / inactive-window fades are out of scope at this stage.
//! Values are the measured light ones and the proposed dark ones.

use iced::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // the table is kept complete per spec §2
pub struct Tokens {
    pub mode: Mode,
    // surfaces
    pub surface_titlebar: Color,
    pub surface_content: Color,
    pub surface_dialog: Color,
    pub divider: Color,
    pub divider_wizard: Color,
    // text
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub text_wizard_header: Color,
    // accent (default blue ramp, fixed at this stage)
    pub accent_fill: Color,
    pub accent_border: Color,
    pub accent_border_bottom: Color,
    pub accent_text: Color,
    pub accent_link: Color,
    pub accent_link_hover: Color,
    pub accent_command_text: Color,
    pub accent_tint_selected: Color,
    pub accent_tint_hover: Color,
    pub accent_tint_border: Color,
    pub text_on_accent: Color,
    // chrome
    pub focus_stroke: Color,
    pub button_fill: Color,
    pub button_border: Color,
    pub button_border_bottom: Color,
    pub button_hover_fill: Color,
    pub button_pressed_fill: Color,
    pub button_pressed_text: Color,
    pub checkbox_border: Color,
    pub checkbox_fill: Color,
    pub textbox_fill: Color,
    pub textbox_border: Color,
    pub glyph_primary: Color,
    pub glyph_disabled: Color,
    pub glyph_dimmed: Color,
    pub chevron: Color,
    pub chevron_disabled: Color,
    pub subtle_hover: Color,
    pub subtle_pressed: Color,
    pub tooltip_fill: Color,
    pub tooltip_border: Color,
    // progress / state colors (semantic, never accent — spec §2.7)
    pub progress_border: Color,
    pub progress_track: Color,
    pub progress_fill: Color,
    pub graph_bg: Color,
    pub graph_done: Color,
    pub graph_done_grid: Color,
    pub graph_speed: Color,
    pub graph_grid: Color,
    pub graph_speed_line: Color,
    pub graph_shimmer: Color,
    pub paused_fill: Color,
    pub paused_done: Color,
    pub paused_grid: Color,
    pub error_fill: Color,
    pub warning_icon: Color,
    pub delete_icon_x: Color,
    pub command_icon_check: Color,
    pub command_icon_undo: Color,
}

pub const ACCENT: u32 = 0x0000_78D4;
pub const ACCENT_DARK1: u32 = 0x0000_5FB8;
pub const ACCENT_DARK2: u32 = 0x0000_3E92;
#[allow(dead_code)]
pub const ACCENT_DARK3: u32 = 0x0000_1A68;
pub const ACCENT_LIGHT1: u32 = 0x0000_93F9;
pub const ACCENT_LIGHT2: u32 = 0x0060_CDFF;
pub const ACCENT_LIGHT3: u32 = 0x0099_EBFF;

impl Tokens {
    pub fn new(mode: Mode) -> Self {
        match mode {
            Mode::Light => Self {
                mode,
                surface_titlebar: rgb(0xF3F3F3),
                surface_content: rgb(0xFFFFFF),
                surface_dialog: rgb(0xF0F0F0),
                divider: rgb(0xE3E3E3),
                divider_wizard: rgb(0xDFDFDF),
                text_primary: rgb(0x000000),
                text_secondary: rgb(0x6D6D6D),
                text_disabled: rgb(0xA0A0A0),
                text_wizard_header: rgb(0x282828),
                accent_fill: rgb(ACCENT_DARK1),
                accent_border: rgb(ACCENT),
                accent_border_bottom: rgb(0x0000_6BBE),
                accent_text: rgb(ACCENT_DARK2),
                accent_link: rgb(0x0000_66CC),
                accent_link_hover: rgb(0x0000_4C99),
                accent_command_text: rgb(0x0015_1C55),
                accent_tint_selected: rgb(0x00CC_E8FF),
                accent_tint_hover: rgb(0x00E5_F3FF),
                accent_tint_border: rgb(0x0099_D1FF),
                text_on_accent: rgb(0xFFFFFF),
                focus_stroke: rgb(0x000000),
                button_fill: rgb(0xFDFDFD),
                button_border: rgb(0xD0D0D0),
                button_border_bottom: rgb(0xBABABA),
                button_hover_fill: rgb(0xF5F5F5),
                button_pressed_fill: rgb(0xEDEDED),
                button_pressed_text: rgb(0x5D5D5D),
                checkbox_border: rgb(0x626262),
                checkbox_fill: rgb(0xF3F3F3),
                textbox_fill: rgb(0xFFFFFF),
                textbox_border: rgb(0xECECEC),
                glyph_primary: rgb(0x000000),
                glyph_disabled: rgb(0xC9C9C9),
                glyph_dimmed: rgb(0x767676),
                chevron: rgb(0x3D3D3D),
                chevron_disabled: rgb(0xB4B4B4),
                subtle_hover: rgb(0xE9E9E9),
                subtle_pressed: rgb(0xE0E0E0),
                tooltip_fill: rgb(0xF9F9F9),
                tooltip_border: rgb(0xCCCCCC),
                progress_border: rgb(0xBCBCBC),
                progress_track: rgb(0xE6E6E6),
                progress_fill: rgb(0x0F7B0F),
                graph_bg: rgb(0xFFFFFF),
                graph_done: rgb(0xA7E591),
                graph_done_grid: rgb(0x8EDD7B),
                graph_speed: rgb(0x06B025),
                graph_grid: rgb(0xCDEFD3),
                graph_speed_line: rgb(0x000000),
                graph_shimmer: rgb(0xDAF3DF),
                paused_fill: rgb(0x8F6E00),
                paused_done: rgb(0xF5E3A3),
                paused_grid: rgb(0xEBD38A),
                error_fill: rgb(0xC42B1C),
                warning_icon: rgb(0xFFC200),
                delete_icon_x: rgb(0xCD3704),
                command_icon_check: rgb(0x45BF48),
                command_icon_undo: rgb(ACCENT),
            },
            Mode::Dark => Self {
                mode,
                surface_titlebar: rgb(0x202020),
                surface_content: rgb(0x2B2B2B),
                surface_dialog: rgb(0x202020),
                divider: rgb(0x3D3D3D),
                divider_wizard: rgb(0x3D3D3D),
                text_primary: rgb(0xFFFFFF),
                text_secondary: rgb(0xC5C5C5),
                text_disabled: rgb(0x787878),
                text_wizard_header: rgb(0xE4E4E4),
                accent_fill: rgb(ACCENT_LIGHT2),
                accent_border: rgb(ACCENT_LIGHT2),
                accent_border_bottom: rgb(ACCENT_LIGHT1),
                accent_text: rgb(ACCENT_LIGHT3),
                accent_link: rgb(ACCENT_LIGHT2),
                accent_link_hover: rgb(ACCENT_LIGHT3),
                accent_command_text: rgb(0xFFFFFF),
                accent_tint_selected: rgb(0x1C4666),
                accent_tint_hover: rgb(0x233949),
                accent_tint_border: rgb(0x2E5F82),
                text_on_accent: rgb(0x000000),
                focus_stroke: rgb(0xFFFFFF),
                button_fill: rgb(0x2D2D2D),
                button_border: rgb(0x454545),
                button_border_bottom: rgb(0x3A3A3A),
                button_hover_fill: rgb(0x323232),
                button_pressed_fill: rgb(0x272727),
                button_pressed_text: rgb(0xC5C5C5),
                checkbox_border: rgb(0x9A9A9A),
                checkbox_fill: rgb(0x2D2D2D),
                textbox_fill: rgb(0x2D2D2D),
                textbox_border: rgb(0x3D3D3D),
                glyph_primary: rgb(0xFFFFFF),
                glyph_disabled: rgb(0x5D5D5D),
                glyph_dimmed: rgb(0xA0A0A0),
                chevron: rgb(0xD0D0D0),
                chevron_disabled: rgb(0x5D5D5D),
                subtle_hover: rgb(0x3A3A3A),
                subtle_pressed: rgb(0x333333),
                tooltip_fill: rgb(0x2C2C2C),
                tooltip_border: rgb(0x1C1C1C),
                progress_border: rgb(0x8A8A8A),
                progress_track: rgb(0x3D3D3D),
                progress_fill: rgb(0x6CCB5F),
                graph_bg: rgb(0x2B2B2B),
                graph_done: rgb(0x2E5E2A),
                graph_done_grid: rgb(0x3F7A3A),
                graph_speed: rgb(0x6CCB5F),
                graph_grid: rgb(0x34433A),
                graph_speed_line: rgb(0xFFFFFF),
                graph_shimmer: rgb(0x2F4A35),
                paused_fill: rgb(0xE5B800),
                paused_done: rgb(0x5C4D14),
                paused_grid: rgb(0x6E5D20),
                error_fill: rgb(0xFF99A4),
                warning_icon: rgb(0xFFC200),
                delete_icon_x: rgb(0xFF7A5C),
                command_icon_check: rgb(0x6CCB5F),
                command_icon_undo: rgb(ACCENT_LIGHT2),
            },
        }
    }
}
