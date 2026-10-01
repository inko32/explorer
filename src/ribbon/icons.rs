//! Builtin SVG icon set.
//!
//! The original framework resolves `IUIImage` from bitmap resources; here
//! every builtin glyph is a small hand-drawn SVG so ribbon buttons keep
//! their function without any binary assets. Icons are monochrome-ish and
//! scale to both 16px (small) and 32px (large) slots.

macro_rules! icon {
    ($name:literal, $body:literal) => {
        (
            $name,
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">"#,
                $body,
                "</svg>"
            ),
        )
    };
}


/// All builtin icons: `(name, svg source)`.
pub const ICONS: &[(&str, &str)] = &[
    icon!("blank", ""),
    // application menu / file
    icon!("new", r##"<path d="M4 1h5l3 3v11H4z" fill="#fff" stroke="#333333"/><path d="M9 1v3h3" fill="none" stroke="#333333"/><path d="M6.2 8h3.6M8 6.2v3.6" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    icon!("open", r##"<path d="M2 4h4l1.2 1.6H14v6.4H2z" fill="#fff" stroke="#333333"/><path d="M2 7h12" stroke="#333333"/>"##),
    icon!("save", r##"<path d="M3 2h8l2 2v10H3z" fill="#fff" stroke="#333333"/><path d="M5 2v4h5V2" fill="none" stroke="#333333"/><rect x="5" y="9" width="6" height="3" fill="#2A6DB5" opacity=".85"/>"##),
    icon!("save_as", r##"<path d="M3 2h8l2 2v6H3z" fill="#fff" stroke="#333333"/><path d="M5 2v4h5V2" fill="none" stroke="#333333"/><path d="M6 13.6h4M8 11.8v3.6" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    icon!("print", r##"<path d="M4 6V2h8v4" fill="#fff" stroke="#333333"/><rect x="2" y="6" width="12" height="5" fill="#DDE6EF" stroke="#333333"/><path d="M4 9h8v5H4z" fill="#fff" stroke="#333333"/><circle cx="12" cy="7.5" r=".8" fill="#2A6DB5"/>"##),
    icon!("print_preview", r##"<rect x="2" y="3" width="12" height="10" fill="#fff" stroke="#333333"/><circle cx="7.4" cy="7.4" r="2.6" fill="none" stroke="#2A6DB5" stroke-width="1.2"/><path d="M9.4 9.4L13 12" stroke="#2A6DB5" stroke-width="1.4"/>"##),
    icon!("close", r##"<rect x="3" y="3" width="10" height="10" fill="#fff" stroke="#333333"/><path d="M5.5 5.5l5 5m0-5l-5 5" stroke="#C0504D" stroke-width="1.4"/>"##),
    icon!("exit", r##"<path d="M3 2h6v3H5v6h4v3H3z" fill="#fff" stroke="#333333"/><path d="M9 8h5m-2-2.4L14.2 8 12 10.4" fill="none" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    // clipboard
    icon!("cut", r##"<circle cx="5" cy="5" r="2.4" fill="none" stroke="#333333" stroke-width="1.3"/><circle cx="5" cy="11" r="2.4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M6.8 6.6L13 13M6.8 9.4L11.4 4.6" stroke="#333333" stroke-width="1.1"/>"##),
    icon!("copy", r##"<rect x="5.5" y="5.5" width="7.5" height="7.5" fill="#fff" stroke="#333333"/><path d="M3 10.5V3h7.5" fill="none" stroke="#333333"/>"##),
    icon!("paste", r##"<path d="M5 2h6v3H5z" fill="#FDF3D7" stroke="#333333"/><path d="M4 3H2.5v11h11V3H11" fill="#fff" stroke="#333333"/><rect x="6" y="6.6" width="4" height="4.6" fill="#E8B53A" opacity=".8" stroke="#333333" stroke-width=".7"/>"##),
    icon!("paste_special", r##"<path d="M4 2h6v3H4z" fill="#FDF3D7" stroke="#333333"/><path d="M3 3H1.5v11h11V3H11" fill="#fff" stroke="#333333"/><path d="M5.5 8.4l2 2 3-3.4" fill="none" stroke="#2A6DB5" stroke-width="1.2"/>"##),
    icon!("undo", r##"<path d="M3 7h7a4 4 0 014 4v1" fill="none" stroke="#333333" stroke-width="1.4"/><path d="M6 3.6L2.6 7 6 10.4" fill="none" stroke="#2A6DB5" stroke-width="1.5"/>"##),
    icon!("redo", r##"<path d="M13 7H6a4 4 0 00-4 4v1" fill="none" stroke="#333333" stroke-width="1.4"/><path d="M10 3.6L13.4 7 10 10.4" fill="none" stroke="#2A6DB5" stroke-width="1.5"/>"##),
    // font
    icon!("bold", r##"<path d="M4.4 2.4h3.4a2.5 2.5 0 010 5H4.4zm0 0V13.6m0-6.2h4a2.7 2.7 0 010 5.4h-4" fill="none" stroke="#333333" stroke-width="1.5"/>"##),
    icon!("italic", r##"<path d="M6.6 2.6h5M4.4 13.4h5M9 2.6l-2 10.8" fill="none" stroke="#333333" stroke-width="1.5"/>"##),
    icon!("underline", r##"<path d="M4.4 2.6v5a3.6 3.6 0 007.2 0v-5" fill="none" stroke="#333333" stroke-width="1.5"/><path d="M4 13.6h8" stroke="#2A6DB5" stroke-width="1.5"/>"##),
    icon!("strike", r##"<path d="M4.4 2.6v5a3.6 3.6 0 007.2 0v-5M2.8 8h10.4" fill="none" stroke="#333333" stroke-width="1.4"/>"##),
    icon!("grow_font", r##"<path d="M3 12.6L6.4 3.4l3.4 9.2M4.2 9.5h4.4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M11 5.6V2.4m0 0L9.8 3.6M11 2.4l1.2 1.2M11 8h2" stroke="#2A6DB5" stroke-width="1.2" fill="none"/>"##),
    icon!("shrink_font", r##"<path d="M3 12.6L6.4 3.4l3.4 9.2M4.2 9.5h4.4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M12.4 3.4L11 2.4l-1.4 1m1.4 4.8V2.4M11 8h2" stroke="#2A6DB5" stroke-width="1.2" fill="none"/>"##),
    icon!("clear_format", r##"<path d="M3 12.6L6.4 3.4l3.4 9.2M4.2 9.5h4.4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M9.6 11l4.4 3.2m0-3.2L9.6 14.2" stroke="#C0504D" stroke-width="1.3" transform="translate(-1 -1)"/>"##),
    icon!("font_color", r##"<path d="M3.4 12.6L6.8 3.4l3.4 9.2" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M2.6 13.8h8.4" stroke="#2A6DB5" stroke-width="2"/>"##),
    icon!("highlight", r##"<path d="M3.4 12.6L6.8 3.4l3.4 9.2" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M2.6 13.8h8.4" stroke="#E8B53A" stroke-width="2"/>"##),
    icon!("subscript", r##"<path d="M2.4 3h5l-4 6.6h4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M9.6 9.6h4.4m-3.8 2.2l3.2 3.4m0-3.4l-3.2 3.4" stroke="#333333" stroke-width="1.1" fill="none"/>"##),
    icon!("superscript", r##"<path d="M2.4 10.6h5l-4-6.6h4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M9.6 2h4.4m-3.8 2.2l3.2 3.4m0-3.4l-3.2 3.4" stroke="#333333" stroke-width="1.1" fill="none"/>"##),
    icon!("change_case", r##"<path d="M2.4 12.4L5.8 3.2l3.4 9.2M3.6 9.3h4.4" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M10.8 6.4a2.3 2.3 0 014.6 0v1h-4.6zm4.6 6.2a2.3 2.3 0 01-4.6 0v-1h4.6z" fill="#fff" stroke="#333333" transform="translate(-2 -1)"/>"##),
    // paragraph
    icon!("align_left", r##"<path d="M2 3h12M2 6.4h8M2 9.8h12M2 13.2h8" stroke="#333333" stroke-width="1.3"/>"##),
    icon!("align_center", r##"<path d="M2 3h12M4 6.4h8M2 9.8h12M4 13.2h8" stroke="#333333" stroke-width="1.3"/>"##),
    icon!("align_right", r##"<path d="M2 3h12M6 6.4h8M2 9.8h12M6 13.2h8" stroke="#333333" stroke-width="1.3"/>"##),
    icon!("justify", r##"<path d="M2 3h12M2 6.4h12M2 9.8h12M2 13.2h12" stroke="#333333" stroke-width="1.3"/>"##),
    icon!("bullet_list", r##"<circle cx="3" cy="4" r="1" fill="#333333"/><circle cx="3" cy="8" r="1" fill="#333333"/><circle cx="3" cy="12" r="1" fill="#333333"/><path d="M6 4h7M6 8h7M6 12h7" stroke="#333333" stroke-width="1.3"/>"##),
    icon!("number_list", r##"<path d="M2.6 3.2v.8h1v1.6h-1V7h2V3.2zm0 5.2v.8h1V11h-1v.8h2V8.4zm0 5.2v.8h1V15h-1v.8h2V13.6z" fill="#333333"/><path d="M6.4 4h6.6M6.4 8h6.6M6.4 12h6.6" stroke="#333333" stroke-width="1.3"/>"##),
    icon!("indent", r##"<path d="M2 3h12M6 6.4h8M6 9.8h8M2 13.2h12" stroke="#333333" stroke-width="1.3"/><path d="M2.8 6.4v4.8M2.8 6.4L4.4 8 2.8 9.6" fill="none" stroke="#2A6DB5" stroke-width="1.2"/>"##),
    icon!("outdent", r##"<path d="M2 3h12M6 6.4h8M6 9.8h8M2 13.2h12" stroke="#333333" stroke-width="1.3"/><path d="M4.4 6.4v4.8M4.4 6.4L2.8 8l1.6 1.6" fill="none" stroke="#2A6DB5" stroke-width="1.2"/>"##),
    // insert
    icon!("table", r##"<rect x="2" y="3" width="12" height="10" fill="#fff" stroke="#333333"/><path d="M2 6.4h12M2 9.8h12M6 3v10M10 3v10" stroke="#333333" fill="none"/>"##),
    icon!("picture", r##"<rect x="2" y="3" width="12" height="10" fill="#fff" stroke="#333333"/><circle cx="5.4" cy="6" r="1.3" fill="#2A6DB5"/><path d="M2 11.4l3.4-3.4 2.4 2.4 2.6-2.8 3.6 3.4" fill="none" stroke="#333333"/>"##),
    icon!("chart", r##"<path d="M2 13.4h12" stroke="#333333"/><rect x="3.4" y="7.4" width="2.2" height="5.4" fill="#2A6DB5"/><rect x="6.9" y="4.6" width="2.2" height="8.2" fill="#2A6DB5" opacity=".8"/><rect x="10.4" y="6.4" width="2.2" height="6.4" fill="#2A6DB5" opacity=".6"/>"##),
    icon!("shapes", r##"<circle cx="5" cy="6" r="3" fill="#DCE6EF" stroke="#333333"/><rect x="7.6" y="7.6" width="5.6" height="5.6" fill="#FDE9D9" stroke="#333333"/><path d="M9.6 2.4l3 3.8h-6z" fill="#EBF1DE" stroke="#333333"/>"##),
    icon!("textbox", r##"<rect x="2" y="4" width="12" height="8" fill="#fff" stroke="#333333"/><path d="M4.4 6.4h7.2M4.4 8.6h5" stroke="#333333" stroke-width="1.1"/>"##),
    icon!("hyperlink", r##"<path d="M6.4 9.6a2.4 2.4 0 003.4 0l2-2a2.42 2.42 0 00-3.4-3.4l-1 1M9.6 6.4a2.4 2.4 0 00-3.4 0l-2 2a2.42 2.42 0 003.4 3.4l1-1" fill="none" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    // editing
    icon!("find", r##"<circle cx="7" cy="7" r="4" fill="none" stroke="#333333" stroke-width="1.4"/><path d="M10 10l3.4 3.4" stroke="#333333" stroke-width="1.6"/>"##),
    icon!("replace", r##"<path d="M2.6 5.6a3.6 3.6 0 013.6-2.2h5.6M11.6 3.4l1.8 1.8-1.8 1.8" fill="none" stroke="#333333" stroke-width="1.2"/><path d="M13.4 10.4a3.6 3.6 0 01-3.6 2.2H4.2m-1.6 0l-1.8-1.8 1.8-1.8" fill="none" stroke="#2A6DB5" stroke-width="1.2"/>"##),
    icon!("select", r##"<path d="M4 2.6l8 5.4-3.6.8 2.2 3.8-1.8 1-2.1-3.7L3.4 12z" fill="#fff" stroke="#333333" stroke-width="1.1"/>"##),
    icon!("zoom_in", r##"<circle cx="7" cy="7" r="4" fill="none" stroke="#333333" stroke-width="1.4"/><path d="M10 10l3.4 3.4M5.4 7h3.2M7 5.4v3.2" stroke="#333333" stroke-width="1.4"/>"##),
    // misc chrome
    icon!("help", r##"<circle cx="8" cy="8" r="6" fill="#fff" stroke="#333333"/><path d="M6.2 6.2a1.9 1.9 0 113 1.5c-.7.5-1.2.9-1.2 1.9" fill="none" stroke="#2A6DB5" stroke-width="1.3"/><circle cx="8" cy="11.6" r=".9" fill="#2A6DB5"/>"##),
    icon!("pin", r##"<path d="M9.6 2.4L13.6 6.4 11.8 8.2 9.2 7.4 6.4 10.2M6.4 10.2L5.4 9.2 8.2 6.4 7.4 3.8z" fill="#fff" stroke="#333333" stroke-width="1.1"/><path d="M4.6 11.2l-2.2 2.2" stroke="#333333" stroke-width="1.4"/>"##),
    icon!("keytip_arrow", r##"<path d="M4 2l6 6-6 6" fill="none" stroke="#333333" stroke-width="1.6"/>"##),
    // explorer file-management commands (ported from the explorer's SVG set)
    icon!("delete", r##"<path d="M2.5 4h11M6.5 2h3M8 2v2M4 5v9.5h8V5" fill="none" stroke="#C0392B" stroke-width="1.4"/><path d="M6 7v5.5M8 7v5.5M10 7v5.5" stroke="#333333" stroke-width="0.8"/>"##),
    icon!("rename", r##"<rect x="1.5" y="4.5" width="13" height="7" fill="none" stroke="#333333" stroke-width="1.2"/><circle cx="5" cy="8" r="1.2" fill="#FFFFFF" stroke="#333333"/>"##),
    icon!("move_to", r##"<rect x="2" y="2" width="8" height="10" fill="#fff" stroke="#333333"/><path d="M9.5 8h5m-1.6-2.4L14.5 8l-1.6 2.4" fill="none" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    icon!("copy_to", r##"<rect x="2" y="2" width="8" height="10" fill="#fff" stroke="#333333"/><rect x="5" y="5" width="8" height="9" fill="#fff" stroke="#333333"/><path d="M12.5 2.6l2 2.4h-4z" fill="#2A6DB5"/>"##),
    icon!("new_folder", r##"<path d="M1 4.5h4.6l1.4 2H15v7.5H1z" fill="#F6C356" stroke="#C88E1E"/><path d="M11 7v5M8.5 9.5h5" stroke="#333333" stroke-width="1.6" fill="none"/>"##),
    icon!("properties", r##"<path d="M2.5 8.5l4 4 7-9" fill="none" stroke="#1F9E3E" stroke-width="2.2"/>"##),
    icon!("select_all", r##"<rect x="2" y="2" width="12" height="12" fill="none" stroke="#333333" stroke-width="1.2"/><path d="M4.5 8.5L7 11l4.5-6.5" fill="none" stroke="#333333" stroke-width="1.4"/>"##),
    icon!("select_none", r##"<rect x="2" y="2" width="12" height="12" fill="none" stroke="#333333" stroke-width="1.2"/>"##),
    icon!("invert_sel", r##"<rect x="2" y="2" width="12" height="12" fill="none" stroke="#333333" stroke-width="1.2"/><rect x="5" y="5" width="6" height="6" fill="#333333"/>"##),
    icon!("view_large", r##"<rect x="2.5" y="2.5" width="11" height="11" fill="#F6C356" stroke="#C88E1E"/>"##),
    icon!("view_details", r##"<circle cx="3" cy="4" r="1" fill="#333333"/><circle cx="3" cy="8" r="1" fill="#333333"/><circle cx="3" cy="12" r="1" fill="#333333"/><path d="M6 4h8M6 8h8M6 12h8" stroke="#333333" stroke-width="1"/>"##),
    icon!("sort_by", r##"<path d="M3 13V3m0 0l3 3M3 3L0 6" fill="none" stroke="#333333" stroke-width="1.3" transform="translate(1.5 1)"/><path d="M10 3v10m0 0l3-3m-3 3l-3-3" fill="none" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    icon!("gear", r##"<circle cx="8" cy="8" r="3" fill="none" stroke="#333333" stroke-width="1.4"/><circle cx="8" cy="8" r="1.2" fill="#333333"/><path d="M8 1v2M8 13v2M1 8h2M13 8h2M3 3l1.4 1.4M11.6 11.6L13 13M13 3l-1.4 1.4M4.4 11.6L3 13" stroke="#333333" stroke-width="1.4"/>"##),
    icon!("edit", r##"<path d="M3 13l1-4L11.5 1.5l2 2L6 11z" fill="#fff" stroke="#333333" stroke-width="1"/>"##),
    icon!("history", r##"<path d="M3 8a5 5 0 115 5" fill="none" stroke="#333333" stroke-width="1.2"/><path d="M8 5v3.5l2.5 1.5" fill="none" stroke="#333333" stroke-width="1.2"/><path d="M2.5 2.5L6 4.5 3 6z" fill="#333333"/>"##),
    icon!("terminal", r##"<rect x="2" y="2" width="12" height="12" fill="#fff" stroke="#333333"/><path d="M4.5 5.5L8 8l-3.5 2.5M8.5 11h3" fill="none" stroke="#333333" stroke-width="1.2"/>"##),
    icon!("nav_pane", r##"<rect x="2" y="2" width="12" height="12" fill="#fff" stroke="#333333"/><path d="M6 2v12" stroke="#333333"/><rect x="3.5" y="3.5" width="1.6" height="1.6" fill="#2A6DB5"/><rect x="3.5" y="6.2" width="1.6" height="1.6" fill="#2A6DB5"/>"##),
    icon!("preview_pane", r##"<rect x="2" y="2" width="12" height="12" fill="#fff" stroke="#333333"/><path d="M10 2v12" stroke="#333333"/><circle cx="5.6" cy="6" r="1.3" fill="#2A6DB5"/><path d="M3.6 10l1.8-2 1.4 1.4 1-1.2 1.8 1.8" fill="none" stroke="#333333" stroke-width="0.8"/>"##),
    icon!("details_pane", r##"<rect x="2" y="2" width="12" height="12" fill="#fff" stroke="#333333"/><path d="M2 10h12M2 6h12" stroke="#333333"/><path d="M5 3.5h6" stroke="#2A6DB5" stroke-width="1.2"/>"##),
    icon!("pictures", r##"<rect x="2" y="3" width="12" height="10" fill="#fff" stroke="#333333"/><circle cx="5.4" cy="6" r="1.3" fill="#2A6DB5"/><path d="M2 11.4l3.4-3.4 2.4 2.4 2.6-2.8 3.6 3.4" fill="none" stroke="#333333"/>"##),
    icon!("email", r##"<rect x="2" y="4" width="12" height="9" fill="#fff" stroke="#333333"/><path d="M2 4.5l6 4.5 6-4.5" fill="none" stroke="#333333"/>"##),
    icon!("extract", r##"<rect x="2" y="6" width="9" height="8" fill="#fff" stroke="#333333"/><path d="M11.5 2v6m0 0L9 5.8m2.5 2.2l2.5-2.4" fill="none" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    // app menu orb (32px slot)
    icon!("orb", r##"<path d="M4 6.2h8M4 8h8M4 9.8h8" stroke="#fff" stroke-width="1.4"/>"##),
];

/// Look up builtin icon source by name.
pub fn get(name: &str) -> Option<&'static str> {
    ICONS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}
