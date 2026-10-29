//! Exercise the real Home renderer with synthetic, in-memory file names only.
//! Layout and interaction tests need no GPU. Set IRIS_RENDER_HOME_TESTS=1 to
//! additionally save CPU-rendered previews of the actual egui meshes. Native
//! window screenshots use tests/native_smoke.py and the application's Glow renderer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[allow(dead_code)]
#[path = "../src/home_view.rs"]
mod home_view;
#[allow(dead_code)]
#[path = "../src/workspace.rs"]
mod workspace;

use egui::{Vec2, accesskit::Role, vec2};
#[allow(dead_code)]
#[path = "../src/archive_view.rs"]
mod archive_view;
#[allow(dead_code)]
#[path = "../src/home_fonts.rs"]
mod home_fonts;
#[path = "../src/theme_button.rs"]
mod theme_button;
#[path = "../src/ui_kit.rs"]
mod ui_kit;
use egui_kittest::{Harness, kittest::Queryable};
use home_view::{Action, Folder, FolderStatus, HomeState};
use workspace::Workspace;
#[path = "support/software_renderer.rs"]
mod software_renderer;

const ROOT: &str = "/Polymath Preview";
const LONG_NAME: &str =
    "Quarterly strategy and operating review — consolidated notes for the next planning cycle.docx";

struct HomeFixture {
    home: HomeState,
    recent: Vec<String>,
    folders: Vec<Folder>,
    dark: bool,
    actions: Vec<Action>,
}

impl HomeFixture {
    fn empty(dark: bool) -> Self {
        Self {
            home: HomeState::default(),
            recent: Vec::new(),
            folders: Vec::new(),
            dark,
            actions: Vec::new(),
        }
    }

    fn populated(dark: bool) -> Self {
        Self {
            recent: [
                LONG_NAME,
                "Roadmap.docx",
                "Budget.xlsx",
                "Launch.pptx",
                "Reference.pdf",
                "Studio portrait.png",
            ]
            .into_iter()
            .map(|name| format!("{ROOT}/Recent work/{name}"))
            .collect(),
            folders: vec![
                Folder {
                    path: format!("{ROOT}/Project briefs"),
                    files: vec![format!("{ROOT}/Project briefs/Workshop outline.docx")],
                    status: FolderStatus::Ready,
                },
                Folder {
                    path: format!(
                        "{ROOT}/Reports and supporting documents with a very long folder name"
                    ),
                    files: vec![format!("{ROOT}/Reports/Yearly results.pdf")],
                    status: FolderStatus::Ready,
                },
            ],
            ..Self::empty(dark)
        }
    }
}

impl eframe::App for HomeFixture {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(action) =
            home_view::show(ui, &mut self.home, &self.recent, &self.folders, self.dark)
        {
            self.actions.push(action);
        }
    }
}

fn harness(size: Vec2, fixture: HomeFixture) -> Harness<'static, HomeFixture> {
    let theme = if fixture.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    let mut harness = Harness::builder()
        .renderer(software_renderer::SoftwareRenderer::default())
        .with_size(size)
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .with_os(egui::os::OperatingSystem::Mac)
        .build_eframe(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            home_fonts::install(&cc.egui_ctx);
            fixture
        });
    harness.run_steps(4);
    harness
}

fn button_present(harness: &Harness<'_, HomeFixture>, label: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Button, label)
        .is_some()
}

fn click(harness: &mut Harness<'_, HomeFixture>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).focus();
    harness.run_steps(12);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);
}

fn capture(harness: &mut Harness<'_, HomeFixture>, name: &str, size: Vec2) {
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/home-design");
    std::fs::create_dir_all(&destination).expect("create generated screenshot directory");
    let image = harness.render().expect("render actual Home UI");
    assert_eq!(
        image.dimensions(),
        ((size.x * 2.0) as u32, (size.y * 2.0) as u32)
    );
    image
        .save(destination.join(format!("{name}.png")))
        .expect("save Home screenshot");
}

#[test]
fn layout_home_light_dark_empty_and_populated_at_desktop_and_minimum_width() {
    let render_snapshots = std::env::var("IRIS_RENDER_HOME_TESTS").as_deref() == Ok("1");
    for (size_label, size) in [
        ("wide", vec2(1152.0, 672.0)),
        ("narrow", vec2(720.0, 800.0)),
    ] {
        for dark in [false, true] {
            for populated in [false, true] {
                let fixture = if populated {
                    HomeFixture::populated(dark)
                } else {
                    HomeFixture::empty(dark)
                };
                let mut harness = harness(size, fixture);
                for label in [
                    "Reader",
                    "PDF Tools",
                    "Writer",
                    "Sheets",
                    "Slides",
                    "Images",
                    "Open a file",
                ] {
                    let rect = harness.get_by_role_and_label(Role::Button, label).rect();
                    assert!(
                        rect.min.x >= 18.0 && rect.max.x <= size.x + 1.0,
                        "{size_label}: {label} overflows: {rect:?}"
                    );
                    if label == "Open a file" {
                        assert!(rect.max.y <= size.y, "{size_label}: Open must stay visible");
                    }
                }
                let compact: Vec<_> = ["Writer", "Sheets", "Slides", "Images"]
                    .iter()
                    .map(|label| harness.get_by_role_and_label(Role::Button, label).rect())
                    .collect();
                for pair in compact.windows(2) {
                    if (pair[0].center().y - pair[1].center().y).abs() < 10.0 {
                        assert!(
                            pair[0].max.x + 8.0 <= pair[1].min.x,
                            "{size_label}: compact workspace cards overlap: {pair:?}"
                        );
                    }
                }
                if populated {
                    let recent = harness
                        .get_by_role_and_label(Role::Button, &format!("Open {LONG_NAME}"))
                        .rect();
                    assert!(
                        recent.max.y <= size.y,
                        "{size_label}: recent work should be visible without scrolling: {recent:?}"
                    );
                    let search = harness.get_by_role(Role::TextInput).rect();
                    assert!(
                        search.min.x >= 18.0 && search.max.x <= size.x + 1.0,
                        "{size_label}: search overflows: {search:?}"
                    );
                    assert!(
                        search.height() <= 24.0,
                        "{size_label}: search field must stay compact: {search:?}"
                    );
                    for label in [
                        "All files",
                        "Reader files",
                        "Documents",
                        "Spreadsheets",
                        "Presentations",
                    ] {
                        let rect = harness.get_by_role_and_label(Role::Button, label).rect();
                        assert!(
                            rect.min.x >= 18.0 && rect.max.x <= size.x + 1.0,
                            "{size_label}: filter {label} overflows: {rect:?}"
                        );
                    }
                }
                if populated {
                    let rect = harness
                        .get_by_role_and_label(Role::Button, LONG_NAME)
                        .rect();
                    let search = harness.get_by_role(Role::TextInput).rect();
                    assert!(
                        rect.min.y - search.max.y <= 80.0,
                        "{size_label}: search container must not expand vertically"
                    );
                    assert!(
                        rect.min.x >= 18.0 && rect.max.x <= size.x + 1.0,
                        "{size_label}: long file name expands the listing: {rect:?}"
                    );
                }
                if render_snapshots {
                    let theme = if dark { "dark" } else { "light" };
                    let contents = if populated { "populated" } else { "empty" };
                    capture(
                        &mut harness,
                        &format!("home-{theme}-{size_label}-{contents}"),
                        size,
                    );
                }
                assert!(harness.state().actions.is_empty());
            }
        }
    }
}

#[test]
fn keyboard_can_enter_workspaces_at_minimum_width() {
    let mut harness = harness(vec2(720.0, 800.0), HomeFixture::empty(true));
    for (label, action) in [
        ("Reader", Action::Enter(Workspace::Reader)),
        ("PDF Tools", Action::Enter(Workspace::Tools)),
        ("Writer", Action::Enter(Workspace::Writer)),
        ("Sheets", Action::Enter(Workspace::Sheets)),
        ("Slides", Action::Enter(Workspace::Slides)),
        ("Images", Action::Enter(Workspace::Images)),
    ] {
        harness.get_by_role_and_label(Role::Button, label).focus();
        harness.run_steps(2);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, label)
                .is_focused()
        );
        harness.key_press(egui::Key::Enter);
        harness.run_steps(3);
        assert_eq!(harness.state_mut().actions.pop(), Some(action));
        assert!(harness.state().actions.is_empty());
    }
}

#[test]
fn search_and_type_filters_update_visible_files_and_can_be_cleared() {
    let mut harness = harness(vec2(1152.0, 800.0), HomeFixture::populated(false));
    assert!(button_present(&harness, "Roadmap.docx"));
    assert!(button_present(&harness, "Budget.xlsx"));

    click(&mut harness, "Recent files");
    let search = harness.get_by_role(Role::TextInput).rect();
    assert!(search.min.y >= 0.0 && search.max.y <= 800.0);
    harness.get_by_role(Role::TextInput).click();
    harness.run_steps(2);
    harness.get_by_role(Role::TextInput).type_text("ROADMAP");
    harness.run_steps(3);
    assert_eq!(harness.state().home.query, "ROADMAP");
    assert!(button_present(&harness, "Roadmap.docx"));
    assert!(!button_present(&harness, "Budget.xlsx"));

    click(&mut harness, "Spreadsheets");
    assert_eq!(harness.state().home.kind, Some(Workspace::Sheets));
    assert!(!button_present(&harness, "Roadmap.docx"));
    assert!(harness.query_by_label("No matching files").is_some());

    click(&mut harness, "Clear filters");
    assert!(harness.state().home.query.is_empty());
    assert_eq!(harness.state().home.kind, None);
    assert!(button_present(&harness, "Roadmap.docx"));
    assert!(button_present(&harness, "Budget.xlsx"));

    click(&mut harness, "Reader files");
    assert!(button_present(&harness, "Reference.pdf"));
    assert!(!button_present(&harness, "Roadmap.docx"));
    click(&mut harness, "Pictures");
    assert!(button_present(&harness, "Studio portrait.png"));
    assert!(!button_present(&harness, "Reference.pdf"));
    assert!(harness.state().actions.is_empty());
}

#[test]
fn selecting_a_folder_changes_listing_and_recent_files_restores_it() {
    let mut harness = harness(vec2(1152.0, 800.0), HomeFixture::populated(true));
    click(&mut harness, "Project briefs");
    assert_eq!(
        harness.state().home.folder.as_deref(),
        Some("/Polymath Preview/Project briefs")
    );
    assert!(button_present(&harness, "Workshop outline.docx"));
    assert!(!button_present(&harness, "Roadmap.docx"));

    click(&mut harness, "Workshop outline.docx");
    assert_eq!(
        harness.state().actions,
        [Action::Open(
            "/Polymath Preview/Project briefs/Workshop outline.docx".into()
        )]
    );

    click(&mut harness, "Recent files");
    assert!(harness.state().home.folder.is_none());
    assert!(button_present(&harness, "Roadmap.docx"));
    assert!(!button_present(&harness, "Workshop outline.docx"));
}

#[test]
fn workspace_cards_return_explicit_actions_without_opening_files() {
    let mut harness = harness(vec2(1152.0, 672.0), HomeFixture::empty(false));
    for (label, action) in [
        ("Reader", Action::Enter(Workspace::Reader)),
        ("PDF Tools", Action::Enter(Workspace::Tools)),
        ("Writer", Action::Enter(Workspace::Writer)),
        ("Sheets", Action::Enter(Workspace::Sheets)),
        ("Slides", Action::Enter(Workspace::Slides)),
        ("Images", Action::Enter(Workspace::Images)),
        ("Open Music", Action::OpenMusicDialog),
        ("Open a file", Action::OpenDialog),
        ("+ Add folder", Action::ChooseFolder),
    ] {
        click(&mut harness, label);
        assert_eq!(harness.state_mut().actions.pop(), Some(action), "{label}");
        assert!(harness.state().actions.is_empty());
    }
    assert!(harness.state().recent.is_empty());
    assert!(harness.state().folders.is_empty());
}

#[test]
fn home_recovers_after_an_editor_replaces_its_font_families() {
    let mut harness = harness(vec2(1152.0, 672.0), HomeFixture::empty(false));
    for dark in [false, true, false] {
        harness.state_mut().dark = dark;
        // Editor activation replaces all fonts. Home must defer drawing its
        // named families until its own definitions become active next pass.
        harness.ctx.set_fonts(egui::FontDefinitions::default());
        harness.run_steps(3);
        assert!(home_fonts::ready(&harness.ctx));
        assert!(button_present(&harness, "Writer"));
    }
}

#[test]
fn theme_icon_is_named_and_keyboard_operable_in_both_modes() {
    let mut harness = Harness::new_ui_state(
        |ui, dark| {
            egui_extras::install_image_loaders(ui.ctx());
            if theme_button::show(ui, *dark).clicked() {
                *dark = !*dark;
            }
        },
        false,
    );
    for label in ["Switch to dark mode", "Switch to light mode"] {
        harness.get_by_role_and_label(Role::Button, label).focus();
        harness.run_steps(2);
        harness.key_press(egui::Key::Enter);
        harness.run_steps(3);
        assert_eq!(*harness.state(), label == "Switch to dark mode");
    }
}

#[test]
fn reduced_motion_keeps_creation_controls_operable_and_stationary() {
    let mut fixture = HomeFixture::empty(false);
    fixture.home.reduce_motion = true;
    let mut harness = harness(vec2(1152.0, 672.0), fixture);
    assert!(harness.state().home.reduce_motion);
    let before = harness.get_by_role_and_label(Role::Button, "Writer").rect();
    harness
        .get_by_role_and_label(Role::Button, "Writer")
        .hover();
    harness.run_steps(3);
    assert_eq!(
        before,
        harness.get_by_role_and_label(Role::Button, "Writer").rect()
    );
    click(&mut harness, "Writer");
    assert_eq!(harness.state().actions, [Action::Enter(Workspace::Writer)]);
}

#[test]
fn keyboard_focus_reveals_creation_control_in_a_short_window() {
    let mut harness = harness(vec2(784.0, 472.0), HomeFixture::empty(false));
    harness
        .get_by_role_and_label(Role::Button, "Slides")
        .focus();
    harness.run_steps(30);
    let rect = harness.get_by_role_and_label(Role::Button, "Slides").rect();
    assert!(
        rect.min.y >= 0.0 && rect.max.y <= 472.0,
        "Focused action must be visible: {rect:?}"
    );
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);
    assert_eq!(harness.state().actions, [Action::Enter(Workspace::Slides)]);
}

#[test]
fn keyboard_focus_reveals_a_file_below_the_fold_in_both_motion_modes() {
    for reduce_motion in [false, true] {
        let mut fixture = HomeFixture::populated(false);
        fixture.home.reduce_motion = reduce_motion;
        let mut harness = harness(vec2(1152.0, 472.0), fixture);
        harness
            .get_by_role_and_label(Role::Button, "Reference.pdf")
            .focus();
        harness.run_steps(if reduce_motion { 3 } else { 30 });
        let rect = harness
            .get_by_role_and_label(Role::Button, "Reference.pdf")
            .rect();
        assert!(
            rect.min.y >= 0.0 && rect.max.y <= 472.0,
            "Focused file must be visible: {rect:?}"
        );
        harness.key_press(egui::Key::Enter);
        harness.run_steps(3);
        assert_eq!(
            harness.state().actions,
            [Action::Open(format!("{ROOT}/Recent work/Reference.pdf"))]
        );
    }
}

#[test]
fn keyboard_focus_returns_to_open_and_reveals_library_controls() {
    for reduce_motion in [false, true] {
        let mut fixture = HomeFixture::populated(false);
        fixture.home.reduce_motion = reduce_motion;
        let mut harness = harness(vec2(784.0, 472.0), fixture);
        // Move far down the page first, then revisit the header and the file controls.
        for label in [
            "Reference.pdf",
            "Open a file",
            "Spreadsheets",
            "+ Add folder",
        ] {
            harness.get_by_role_and_label(Role::Button, label).focus();
            harness.run_steps(if reduce_motion { 3 } else { 30 });
            let rect = harness.get_by_role_and_label(Role::Button, label).rect();
            assert!(
                rect.min.y >= 0.0 && rect.max.y <= 472.0,
                "Focused {label} must stay visible: {rect:?}"
            );
        }
        let name = "Reports and supporting documents with a very long folder name";
        let rect = harness.get_by_role_and_label(Role::Button, name).rect();
        assert!(
            rect.width() <= 241.0,
            "Long folder selector must stay compact: {rect:?}"
        );
        assert!(harness.state().actions.is_empty());
    }
}
