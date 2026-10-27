//! Theme action shared by the native shell and its UI tests.
use egui::{Color32, Sense, Stroke, StrokeKind, vec2};

/// The shell's compact icon control. Keep hover, selected, focus and labels
/// consistent even when a sibling editor changes egui's global widget style.
pub fn icon(
    ui: &mut egui::Ui,
    name: &str,
    label: &str,
    dark: bool,
    selected: bool,
    accent: Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let chrome = crate::ui_kit::chrome(dark);
    let background = if selected {
        chrome
            .surface
            .lerp_to_gamma(accent, if dark { 0.24 } else { 0.12 })
    } else if response.hovered() || response.has_focus() {
        chrome.hover
    } else {
        Color32::TRANSPARENT
    };
    if background != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 8, background);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            9,
            Stroke::new(1.5, accent),
            StrokeKind::Outside,
        );
    }
    let ink = if selected { accent } else { chrome.ink };
    pdfcraft_ui_egui::icons::paint(ui, rect, name, 18.0, ink);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}

pub fn show(ui: &mut egui::Ui, dark: bool) -> egui::Response {
    let label = if dark {
        "Switch to light mode"
    } else {
        "Switch to dark mode"
    };
    icon(
        ui,
        if dark { "sun" } else { "moon" },
        label,
        dark,
        false,
        crate::ui_kit::chrome(dark).ink,
    )
}
