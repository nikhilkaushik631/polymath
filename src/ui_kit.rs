//! Shared controls from the local egui 0.36 port of egui-shadcn.
use crate::Workspace;
use egui::Color32;
use egui_shadcn::{
    Theme,
    tokens::{ColorPalette, DEFAULT_FOCUS, DEFAULT_MOTION, DEFAULT_RADIUS},
};

/// Neutral chrome shared by the host bar, Home, and read-only viewers.
/// Document canvases keep their own colors; workspace accents identify actions.
#[derive(Clone, Copy)]
pub(crate) struct Chrome {
    pub canvas: Color32,
    pub surface: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub hover: Color32,
}

pub(crate) fn chrome(dark: bool) -> Chrome {
    let gray = Color32::from_gray;
    if dark {
        Chrome {
            canvas: gray(22),
            surface: gray(35),
            ink: gray(240),
            muted: gray(172),
            line: gray(62),
            hover: gray(49),
        }
    } else {
        Chrome {
            canvas: gray(250),
            surface: Color32::WHITE,
            ink: gray(25),
            muted: gray(102),
            line: gray(218),
            hover: gray(237),
        }
    }
}

pub(crate) fn theme(dark: bool, workspace: Workspace) -> Theme {
    let chrome = chrome(dark);
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
    palette.background = chrome.canvas;
    palette.foreground = chrome.ink;
    palette.card = chrome.surface;
    palette.card_foreground = palette.foreground;
    palette.border = chrome.line;
    palette.secondary = chrome.hover;
    palette.secondary_foreground = palette.foreground;
    palette.muted = palette.secondary;
    palette.muted_foreground = chrome.muted;
    Theme {
        palette,
        motion: DEFAULT_MOTION,
        radius: DEFAULT_RADIUS,
        focus: DEFAULT_FOCUS,
    }
}
