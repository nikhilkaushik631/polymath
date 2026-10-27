//! Shared controls from the local egui 0.36 port of egui-shadcn.
use crate::Workspace;
use egui::Color32;
use egui_shadcn::{
    Theme,
    tokens::{ColorPalette, DEFAULT_FOCUS, DEFAULT_MOTION, DEFAULT_RADIUS},
};

pub(crate) fn theme(dark: bool, workspace: Workspace) -> Theme {
    let mut palette = if dark {
        ColorPalette::dark()
    } else {
        ColorPalette::light()
    };
    // Keep the upstream component states, but map its semantic colors to
    // Polymath's neutral surfaces and the active workspace's accent.
    let accent = if workspace == Workspace::Home {
        Color32::from_gray(if dark { 235 } else { 29 })
    } else {
        workspace.accent(dark)
    };
    palette.primary = accent;
    palette.primary_foreground = if dark {
        Color32::from_gray(22)
    } else {
        Color32::WHITE
    };
    palette.ring = accent;
    palette.background = Color32::from_gray(if dark { 22 } else { 250 });
    palette.foreground = Color32::from_gray(if dark { 240 } else { 25 });
    palette.card = Color32::from_gray(if dark { 35 } else { 255 });
    palette.card_foreground = palette.foreground;
    palette.border = Color32::from_gray(if dark { 68 } else { 214 });
    palette.secondary = Color32::from_gray(if dark { 42 } else { 237 });
    palette.secondary_foreground = palette.foreground;
    palette.muted = palette.secondary;
    palette.muted_foreground = Color32::from_gray(if dark { 172 } else { 95 });
    Theme {
        palette,
        motion: DEFAULT_MOTION,
        radius: DEFAULT_RADIUS,
        focus: DEFAULT_FOCUS,
    }
}
