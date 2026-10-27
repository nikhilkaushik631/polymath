//! Theme action shared by the native shell and its UI tests.
use egui::{Color32, Sense, Stroke, vec2};

pub fn show(ui: &mut egui::Ui, dark: bool) -> egui::Response {
    let label = if dark {
        "Switch to light mode"
    } else {
        "Switch to dark mode"
    };
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let ink = Color32::from_gray(if dark { 238 } else { 28 });
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 7, Color32::from_gray(if dark { 58 } else { 226 }));
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 7, Stroke::new(1.0, ink), egui::StrokeKind::Inside);
    }
    pdfcraft_ui_egui::icons::paint(ui, rect, if dark { "sun" } else { "moon" }, 20.0, ink);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}
