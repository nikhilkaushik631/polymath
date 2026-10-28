//! Polymath: document and image editors in one native window.
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
mod audio;
mod automation;
mod control;
mod file_dialogs;
mod global_dictation;
mod home;
mod home_fonts;
mod home_view;
mod image_view;
mod local_folders;
mod media_queue;
mod media_view;
mod office_preview;
mod print;
mod reader_memory;
mod reader_tabs;
mod services;
mod settings;
mod source_view;
mod storage;
mod theme_button;
mod ui_kit;
mod video_editor;
mod video_pipeline;
mod voice;
mod workspace;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    sync::mpsc::{Receiver, Sender},
    time::Instant,
};
use workspace::Workspace;

struct Shot {
    reply: Sender<Value>,
    path: Option<String>,
    after: u32,
    started: Instant,
    issued: bool,
}
struct Polymath {
    reader: pdfcraft_ui_egui::PdfCraftApp,
    source: Option<source_view::SourceFile>,
    media: Option<media_view::MediaView>,
    image_preview: Option<image_view::ImageView>,
    active_preview_id: Option<u64>,
    inactive_reader_previews: Vec<reader_tabs::PreviewSlot>,
    reader_tab_order: Vec<reader_tabs::TabId>,
    next_reader_preview_id: u64,
    office_previews: HashMap<pdfcraft_engine::DocId, String>,
    writer: wordcraft_ui_egui::WordApp,
    sheets: gridcraft_ui_egui::SheetApp,
    slides: deckcraft_ui_egui::SlideApp,
    images: lightcraft_ui_egui::LightcraftApp,
    images_error: Option<String>,
    video_editor: video_editor::VideoEditor,
    active: Workspace,
    next: Option<Workspace>,
    dark: bool,
    recent: Vec<String>,
    recent_checked_at: Instant,
    reading_memory: reader_memory::ReaderMemory,
    reading_memory_dirty: bool,
    reading_memory_saved_at: Instant,
    editor_recent_snapshots: [Vec<String>; 4],
    folders: Vec<String>,
    listing: local_folders::LocalFolders,
    home_state: home_view::HomeState,
    notice: Option<String>,
    external_link: Option<String>,
    voice: voice::Voice,
    global_dictation: global_dictation::GlobalDictation,
    settings_page: settings::Page,
    quit_requested: bool,
    force_quit: bool,
    #[cfg(target_os = "macos")]
    native_quit_scheduled: bool,
    dialogs: file_dialogs::Launcher,
    picker: Option<(bool, bool, Receiver<Option<String>>)>,
    server: Option<control::Server>,
    pdf_control: pdfcraft_ui_egui::control::ControlClient,
    pdf_pending: Vec<(
        Receiver<pdfcraft_ui_egui::control::Reply>,
        Sender<Value>,
        Instant,
    )>,
    shots: Vec<Shot>,
    #[cfg(target_os = "macos")]
    apple: fmv_macos_events::Inbox,
}
impl Polymath {
    fn new(
        ctx: &egui::Context,
        #[cfg(target_os = "macos")] apple: fmv_macos_events::Inbox,
    ) -> Self {
        egui_extras::install_image_loaders(ctx);
        home_fonts::install(ctx);
        ctx.set_zoom_factor(1.25);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (hook, dialogs) = file_dialogs::hook(ctx);
        let mut reader = pdfcraft_ui_egui::PdfCraftApp::new();
        reader.os_key_store_ids = cfg!(target_os = "macos");
        reader.host_external_links = true;
        let pdf_control = reader.attach_control(ctx);
        let mut image_session = lightcraft_engine::Session::new()
            .with_fs()
            .with_system_clock();
        let images_error = if cfg!(test)
            || std::env::var_os("POLYMATH_NO_PREFS").is_some()
            || std::env::var_os("IRIS_NO_PREFS").is_some()
        {
            None
        } else if let Some(home) = std::env::var_os("HOME") {
            let library = std::path::PathBuf::from(home)
                .join("Library/Application Support/Polymath/Images Library");
            image_session
                .open_library(&library, false)
                .err()
                .map(|error| {
                    format!(
                        "Could not open the Images library at {}: {error}",
                        library.display()
                    )
                })
        } else {
            Some("Could not locate your home folder for the Images library.".into())
        };
        let mut images =
            lightcraft_ui_egui::LightcraftApp::new(image_session, services::images(ctx));
        images.embedded_host = true;
        images.ui.left_width = 200.0;
        images.ui.right_width = 250.0;
        let mut writer = wordcraft_ui_egui::WordApp::new(
            wordcraft_engine::Session::new(wordcraft_doc::Document::new()),
            services::writer(hook),
        );
        writer.host_external_links = true;
        let mut sheets =
            gridcraft_ui_egui::SheetApp::new(gridcraft_engine::Session::new(), services::sheets());
        sheets.host_external_links = true;
        sheets.embedded_host = true;
        let mut slides =
            deckcraft_ui_egui::SlideApp::new(deckcraft_engine::Session::new(), services::slides());
        slides.host_external_links = true;
        slides.embedded_host = true;
        let mut app = Self {
            reader,
            source: None,
            media: None,
            image_preview: None,
            active_preview_id: None,
            inactive_reader_previews: Vec::new(),
            reader_tab_order: Vec::new(),
            next_reader_preview_id: 1,
            office_previews: HashMap::new(),
            writer,
            sheets,
            slides,
            images,
            images_error,
            video_editor: video_editor::VideoEditor::new(),
            active: Workspace::Home,
            next: None,
            dark: false,
            recent: vec![],
            recent_checked_at: Instant::now(),
            reading_memory: reader_memory::ReaderMemory::default(),
            reading_memory_dirty: false,
            reading_memory_saved_at: Instant::now(),
            editor_recent_snapshots: Default::default(),
            folders: vec![],
            listing: Default::default(),
            home_state: home_view::HomeState {
                reduce_motion: home::system_reduce_motion(),
                ..Default::default()
            },
            notice: None,
            external_link: None,
            voice: voice::Voice::default(),
            global_dictation: global_dictation::GlobalDictation::default(),
            settings_page: settings::Page::default(),
            quit_requested: false,
            force_quit: false,
            #[cfg(target_os = "macos")]
            native_quit_scheduled: false,
            dialogs,
            picker: None,
            server: None,
            pdf_control,
            pdf_pending: vec![],
            shots: vec![],
            #[cfg(target_os = "macos")]
            apple,
        };
        if !cfg!(test) {
            app.load_preferences();
            if (std::env::var_os("POLYMATH_NO_PREFS").is_none()
                && std::env::var_os("IRIS_NO_PREFS").is_none())
                && let Some(home) = std::env::var_os("HOME")
            {
                let support = std::path::PathBuf::from(home).join("Library/Application Support");
                let recovery = support.join("Polymath/recovery");
                let legacy = support.join("Iris Suite/recovery");
                let recovery = if !recovery.exists() && legacy.exists() {
                    legacy
                } else {
                    recovery
                };
                app.reader
                    .enable_recovery(pdfcraft_ui_egui::RecoveryStore::new(
                        recovery.join("reader"),
                    ));
                app.slides.session.recovery_dir = Some(recovery.join("slides"));
                if let Err(e) = app.slides.session.execute("file.recovery.open", &json!({})) {
                    app.notice = Some(format!("Presentation recovery: {e}"));
                }
            }
        }
        let recent_pruned = app.prune_missing_recent();
        app.sync_editor_recent(false);
        if recent_pruned {
            app.save_preferences();
        }
        app.apply_theme();
        app.home_theme(ctx);
        if app.global_dictation.enabled {
            app.global_dictation
                .set_enabled(true, ctx, &app.voice.language);
        }
        app
    }

    fn queue_external_link(&mut self, address: String) {
        match pdfcraft_engine::links::document_url(&address) {
            Ok(address) => self.external_link = Some(address),
            Err(error) => self.notice = Some(format!("This document link cannot open: {error}")),
        }
    }
    fn dirty(&self, w: Workspace) -> bool {
        match w {
            Workspace::Home => false,
            Workspace::Reader => self.reader.first_dirty().is_some(),
            Workspace::Tools => false,
            Workspace::Writer => self.writer.session.dirty,
            Workspace::Sheets => {
                self.sheets.editor.is_some()
                    || self.sheets.text_box_editor.is_some()
                    || self.sheets.session.documents().iter().any(|d| d.is_dirty())
            }
            Workspace::Slides => self.slides.session.documents().iter().any(|d| d.is_dirty()),
            Workspace::Images => {
                self.images.session.unsaved().is_some()
                    || !self
                        .images
                        .session
                        .activity
                        .running_cancellable()
                        .is_empty()
            }
            Workspace::Video => {
                self.video_editor.has_unsaved_work() || self.video_editor.has_export_running()
            }
        }
    }
    fn dirty_workspaces(&self) -> Vec<Workspace> {
        Workspace::ALL
            .into_iter()
            .filter(|w| self.dirty(*w))
            .collect()
    }
    fn activate(&mut self, w: Workspace, ctx: &egui::Context) {
        if self.active == w {
            return;
        }
        if self.active == Workspace::Reader && w != Workspace::Reader {
            if let Some(media) = &mut self.media {
                media.pause();
                if media.fullscreen {
                    let _ = media.set_fullscreen(ctx, false);
                }
            }
            if let Some(image) = &mut self.image_preview
                && image.fullscreen
            {
                image.set_fullscreen(ctx, false);
            }
        }
        self.active = w;
        if matches!(
            w,
            Workspace::Home
                | Workspace::Reader
                | Workspace::Tools
                | Workspace::Sheets
                | Workspace::Images
                | Workspace::Video
        ) {
            ctx.set_zoom_factor(1.25);
        }
        ctx.all_styles_mut(|style| *style = egui::Style::default());
        // Each editor installs its own named fonts. Reinstall only on activation, never rebuild sessions.
        match w {
            Workspace::Home => {
                home_fonts::install(ctx);
                self.home_theme(ctx);
            }
            Workspace::Reader => {
                self.reader.host_read_only = true;
                self.reader.host_tabs = true;
                self.reader.host_tools_workspace = false;
                self.reader.mode = pdfcraft_ui_egui::Mode::Read;
                self.reader.default_mode = pdfcraft_ui_egui::Mode::Read;
                if self.source.is_some() || self.media.is_some() || self.image_preview.is_some() {
                    home_fonts::install(ctx);
                    self.home_theme(ctx);
                } else {
                    self.reader.activate_in_host();
                }
            }
            Workspace::Tools => {
                self.reader.host_read_only = false;
                self.reader.host_tabs = false;
                self.reader.host_tools_workspace = true;
                self.reader.mode = pdfcraft_ui_egui::Mode::AllTools;
                self.reader.default_mode = pdfcraft_ui_egui::Mode::AllTools;
                self.reader.left = pdfcraft_ui_egui::LeftPanel::AllTools;
                self.reader.left_open = true;
                self.reader.all_tools_expanded = true;
                self.reader.activate_in_host();
            }
            Workspace::Writer => self.writer.activate_in_host(),
            Workspace::Sheets => self.sheets.activate_in_host(),
            Workspace::Slides => self.slides.activate_in_host(),
            Workspace::Images => self.images.activate_in_host(ctx),
            Workspace::Video => {
                home_fonts::install(ctx);
                self.home_theme(ctx);
            }
        }
        ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(egui::Id::NULL)));
        ctx.request_repaint();
    }
    fn apply_theme(&mut self) {
        use deckcraft_ui_egui::theme::Brightness as S;
        use pdfcraft_ui_egui::theme::ThemePreference as P;
        use wordcraft_ui_egui::theme::Appearance as W;
        self.reader
            .set_theme_preference(if self.dark { P::Dark } else { P::Light });
        self.writer.ui.theme = if self.dark { W::Dark } else { W::Light };
        self.sheets.ui.dark = self.dark;
        self.sheets.ui.system_theme = false;
        self.slides.ui.brightness = if self.dark { S::Dark } else { S::Light };
        self.images.ui.settings.select_theme(if self.dark {
            lightcraft_ui_egui::theme::ThemeKind::Charcoal
        } else {
            lightcraft_ui_egui::theme::ThemeKind::Paper
        });
        self.slides.activate_in_host();
    }
    fn set_dark(&mut self, dark: bool, ctx: &egui::Context) {
        self.dark = dark;
        self.apply_theme();
        match self.active {
            Workspace::Home | Workspace::Video => self.home_theme(ctx),
            Workspace::Reader
                if self.source.is_some()
                    || self.media.is_some()
                    || self.image_preview.is_some() =>
            {
                self.home_theme(ctx)
            }
            Workspace::Images => self.images.sync_theme(ctx),
            _ => {}
        }
        ctx.request_repaint();
    }
    fn home_theme(&self, ctx: &egui::Context) {
        let chrome = ui_kit::chrome(self.dark);
        ctx.set_theme(if self.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        let mut style = egui::Style {
            visuals: if self.dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            },
            ..Default::default()
        };
        style.visuals.override_text_color = Some(chrome.ink);
        style.visuals.weak_text_color = Some(chrome.muted);
        style.visuals.panel_fill = chrome.canvas;
        style.visuals.window_fill = chrome.surface;
        style.visuals.widgets.inactive.weak_bg_fill = chrome.surface;
        style.visuals.widgets.hovered.weak_bg_fill = chrome.hover;
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, chrome.line);
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, chrome.line);
        style.visuals.selection.bg_fill =
            egui::Color32::from_gray(if self.dark { 80 } else { 210 });
        style.visuals.selection.stroke = egui::Stroke::new(1.0, style.visuals.text_color());
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
        ctx.set_style_of(
            if self.dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            },
            style,
        );
    }
    fn open(&mut self, path: &str, ctx: &egui::Context) -> Result<Value, String> {
        // Every ordinary open enters a visible Reader tab. Editing is an explicit handoff.
        self.open_in_reader(path, ctx)
    }
    fn open_recent_or_prune(&mut self, path: &str, ctx: &egui::Context) -> Result<(), String> {
        if self.recent.iter().any(|recent| recent == path) && !Path::new(path).is_file() {
            if self.prune_missing_recent() {
                self.sync_editor_recent(false);
                self.save_preferences();
            }
            return Ok(());
        }
        self.open(path, ctx).map(|_| ())
    }
    fn open_for_edit(&mut self, path: &str, ctx: &egui::Context) -> Result<Value, String> {
        if workspace::is_video_path(Path::new(path)) {
            self.video_editor.add_clip_path(path.into(), ctx)?;
            self.activate(Workspace::Video, ctx);
            self.recent.retain(|recent| recent != path);
            self.recent.insert(0, path.to_owned());
            self.recent.truncate(16);
            return Ok(json!({"workspace":"video","requested":path}));
        }
        self.open_routed(path, ctx, true)
    }
    fn open_routed(
        &mut self,
        path: &str,
        ctx: &egui::Context,
        edit: bool,
    ) -> Result<Value, String> {
        let p = Path::new(path);
        if !p.is_file() {
            self.recent.retain(|item| item != path);
            return Err(format!("File not found: {path}"));
        }
        let route = Workspace::for_path(p);
        let is_source = p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            workspace::SOURCE_EXTENSIONS
                .iter()
                .any(|known| e.eq_ignore_ascii_case(known))
        });
        let is_media = p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            workspace::MEDIA_EXTENSIONS
                .iter()
                .any(|known| e.eq_ignore_ascii_case(known))
        });
        let image_preview = if !edit && matches!(&route, Ok(Workspace::Images)) {
            Some(image_view::ImageView::open(p)?)
        } else {
            None
        };
        let text_preview = !edit
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("txt"));
        let source = if !is_media && (is_source || route.is_err() || text_preview) {
            Some(source_view::SourceFile::load(p)?)
        } else {
            None
        };
        let w = if source.is_some() || image_preview.is_some() {
            Workspace::Reader
        } else {
            route?
        };
        let media = if is_media {
            Some(media_view::MediaView::open(p)?)
        } else {
            None
        };
        if w == Workspace::Images
            && let Some(error) = &self.images_error
        {
            return Err(error.clone());
        }
        if w == Workspace::Reader {
            if let Some(source) = source {
                self.open_reader_preview(reader_tabs::Preview::Source(source), ctx);
            } else if let Some(media) = media {
                self.open_reader_preview(reader_tabs::Preview::Media(media), ctx);
            } else if let Some(image) = image_preview {
                self.open_reader_preview(reader_tabs::Preview::Image(image), ctx);
            } else {
                self.stash_reader_preview(ctx);
                self.activate(Workspace::Reader, ctx);
                if self.active == Workspace::Reader {
                    self.reader.activate_in_host();
                }
                self.reader.open_path(path);
                self.sync_reader_tabs();
            }
        } else {
            // Switching to an editor retains all other open Reader and editor state.
            self.activate(w, ctx);
            match w {
                Workspace::Writer => {
                    self.writer.run("file.open", json!({"path":path}))?;
                    self.writer.ui.backstage = false;
                }
                Workspace::Sheets => {
                    self.sheets.run("file.open", json!({"path":path}))?;
                }
                Workspace::Slides => self.slides.open_path(path)?,
                Workspace::Images => {
                    lightcraft_ui_egui::import::start_paths(&mut self.images, vec![path.into()])?;
                    self.images.ui.view = lightcraft_ui_egui::state::ViewMode::Detail;
                }
                Workspace::Home | Workspace::Reader | Workspace::Tools | Workspace::Video => {}
            }
        }
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.into());
        self.recent.truncate(16);
        Ok(json!({"workspace":w,"requested":path}))
    }
    fn open_in_reader(&mut self, path: &str, ctx: &egui::Context) -> Result<Value, String> {
        let file = Path::new(path);
        if !file.is_file() {
            self.recent.retain(|item| item != path);
            return Err(format!("File not found: {path}"));
        }
        if self.select_reader_path(path, ctx) {
            self.recent.retain(|item| item != path);
            self.recent.insert(0, path.to_owned());
            self.recent.truncate(16);
            return Ok(json!({"workspace":"reader","requested":path,"readOnly":true}));
        }
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "pdf" {
            let mut opened = self.open_routed(path, ctx, false)?;
            opened["readOnly"] = json!(true);
            return Ok(opened);
        }
        if workspace::MEDIA_EXTENSIONS.contains(&ext.as_str()) {
            let mut opened = self.open_routed(path, ctx, false)?;
            opened["readOnly"] = json!(true);
            return Ok(opened);
        }
        if Workspace::for_path(file) == Ok(Workspace::Images) {
            let mut opened = self.open_routed(path, ctx, false)?;
            opened["readOnly"] = json!(true);
            return Ok(opened);
        }
        if matches!(
            ext.as_str(),
            "docx"
                | "docm"
                | "dotx"
                | "dotm"
                | "doc"
                | "dot"
                | "odt"
                | "rtf"
                | "xlsx"
                | "xlsm"
                | "xltx"
                | "xltm"
                | "xlsb"
                | "ods"
                | "pptx"
                | "potx"
                | "ppsx"
                | "deckcraft"
        ) {
            let pdf = office_preview::pdf_bytes(file)?;
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Document");
            self.stash_reader_preview(ctx);
            self.reader.host_read_only = true;
            self.reader.host_tabs = true;
            self.reader
                .open_bytes(&format!("{name} · preview.pdf"), None, pdf)?;
            let (_, id) = self
                .reader
                .active_ids()
                .ok_or("The office preview did not open")?;
            self.office_previews.insert(id, path.to_owned());
            self.sync_reader_tabs();
            if self.active == Workspace::Reader {
                self.reader.activate_in_host();
            } else {
                self.activate(Workspace::Reader, ctx);
            }
        } else {
            self.open_reader_preview(
                reader_tabs::Preview::Source(source_view::SourceFile::load(file)?),
                ctx,
            );
        }
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_owned());
        self.recent.truncate(16);
        Ok(json!({"workspace":"reader","requested":path,"readOnly":true}))
    }
    /// The embedded editors can open and save files without going through Polymath's Open action.
    /// Bring those paths into Home without changing the order of already recorded activity.
    fn sync_editor_recent(&mut self, promote_changes: bool) {
        let lists = [
            self.reader
                .recent
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            self.writer.ui.recent.clone(),
            self.sheets.ui.recent.clone(),
            self.slides.ui.recent.clone(),
        ];
        for (index, paths) in lists.into_iter().enumerate() {
            let previous = &self.editor_recent_snapshots[index];
            if promote_changes && previous == &paths {
                continue;
            }
            let changed = previous.first() != paths.first();
            self.editor_recent_snapshots[index] = paths.clone();
            for path in paths {
                if Path::new(&path).is_file() && !self.recent.contains(&path) {
                    self.recent.push(path);
                }
            }
            if promote_changes
                && changed
                && let Some(path) = self.editor_recent_snapshots[index].first()
                && Path::new(path).is_file()
            {
                self.recent.retain(|recent| recent != path);
                self.recent.insert(0, path.clone());
            }
        }
        self.recent.truncate(16);
    }
    fn editable_reader_path(&self) -> Option<String> {
        if let Some(image) = &self.image_preview {
            return Some(image.path.clone());
        }
        self.active_office_preview_path()
            .map(str::to_owned)
            .or_else(|| {
                self.source.as_ref().and_then(|source| {
                    let path = Path::new(&source.path);
                    matches!(
                        Workspace::for_path(path),
                        Ok(Workspace::Writer | Workspace::Sheets | Workspace::Slides)
                    )
                    .then(|| source.path.clone())
                })
            })
    }
    fn current_reader_path(&self) -> Option<String> {
        if let Some(image) = &self.image_preview {
            return Some(image.path.clone());
        }
        if let Some(media) = &self.media {
            return Some(media.path.clone());
        }
        if let Some(source) = &self.source {
            return Some(source.path.clone());
        }
        let (_, id) = self.reader.active_ids()?;
        self.office_previews
            .get(&id)
            .cloned()
            .or_else(|| self.reader.session.get(id).and_then(|doc| doc.path.clone()))
    }
    fn active_office_preview_path(&self) -> Option<&str> {
        if self.source.is_some() || self.media.is_some() || self.image_preview.is_some() {
            return None;
        }
        let (_, id) = self.reader.active_ids()?;
        self.office_previews.get(&id).map(String::as_str)
    }
    fn pick(&mut self, folder: bool, frame: &eframe::Frame, ctx: &egui::Context) {
        self.pick_filtered(folder, frame, ctx, workspace::EXTENSIONS);
    }
    fn pick_filtered(
        &mut self,
        folder: bool,
        frame: &eframe::Frame,
        ctx: &egui::Context,
        extensions: &'static [&'static str],
    ) {
        if self.picker.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let dialog = rfd::AsyncFileDialog::new()
            .set_parent(frame)
            .set_title(if folder {
                "Choose a folder for Polymath"
            } else {
                "Open in Polymath"
            });
        let wake = ctx.clone();
        let future: std::pin::Pin<
            Box<dyn std::future::Future<Output = Option<rfd::FileHandle>> + Send>,
        > = if folder {
            Box::pin(dialog.pick_folder())
        } else {
            Box::pin(
                dialog
                    .add_filter("Supported files", extensions)
                    .add_filter("All files", &["*"])
                    .pick_file(),
            )
        };
        match std::thread::Builder::new()
            .name("Polymath file picker".into())
            .spawn(move || {
                let _ = tx.send(
                    pollster::block_on(future).map(|f| f.path().to_string_lossy().into_owned()),
                );
                wake.request_repaint();
            }) {
            Ok(_) => self.picker = Some((folder, self.active == Workspace::Reader, rx)),
            Err(e) => self.notice = Some(e.to_string()),
        }
    }
    fn poll_picker(&mut self, ctx: &egui::Context) {
        let answer = self
            .picker
            .as_ref()
            .and_then(|(folder, preview, rx)| match rx.try_recv() {
                Ok(v) => Some((*folder, *preview, v)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some((*folder, *preview, None)),
                _ => None,
            });
        if let Some((folder, preview, path)) = answer {
            self.picker = None;
            if let Some(path) = path {
                let result = if folder {
                    local_folders::add(&mut self.folders, path).map(|_| ())
                } else if preview {
                    self.open_in_reader(&path, ctx).map(|_| ())
                } else {
                    self.open(&path, ctx).map(|_| ())
                };
                if let Err(e) = result {
                    self.notice = Some(e);
                }
            }
        }
    }
    fn summary(&self) -> Value {
        json!({"application":"Polymath","workspace":self.active,"dark":self.dark,"dirtyWorkspaces":self.dirty_workspaces(),"quitConfirmation":self.quit_requested,"localFolders":self.folders,"recent":self.recent,"readerTabs":self.reader_tabs_json(),"source":self.source.as_ref().map(|file| json!({"path":file.path,"language":file.language,"lines":file.line_count()})),"imagePreview":self.image_preview.as_ref().map(|image| image.inspect()),"media":self.media.as_ref().map(|media| media.inspect()),"videoEditor":self.video_editor.inspect(),"editors":{"reader":self.reader.session.docs().len(),"writer":1,"sheets":self.sheets.session.documents().len(),"slides":self.slides.session.documents().len(),"images":self.images.session.catalog.len(),"video":self.video_editor.inspect()["clipCount"]},"imagesError":self.images_error})
    }
}
impl eframe::App for Polymath {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if !self.force_quit {
            for viewport in raw.viewports.values_mut() {
                viewport.events.retain(|e| {
                    if matches!(e, egui::ViewportEvent::Close) {
                        self.quit_requested = true;
                        false
                    } else {
                        true
                    }
                });
            }
            if self.quit_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        let mut switch = None;
        let mut open = false;
        raw.events.retain(|e| {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = e
                && modifiers.command
            {
                match key {
                    egui::Key::Q => {
                        self.quit_requested = true;
                        return false;
                    }
                    egui::Key::O => {
                        open = true;
                        return false;
                    }
                    egui::Key::Comma => {
                        self.voice.panel_open = true;
                        return false;
                    }
                    egui::Key::Num0 => switch = Some(Workspace::Home),
                    egui::Key::Num1 => switch = Some(Workspace::Reader),
                    egui::Key::Num2 => switch = Some(Workspace::Writer),
                    egui::Key::Num3 => switch = Some(Workspace::Sheets),
                    egui::Key::Num4 => switch = Some(Workspace::Slides),
                    egui::Key::Num5 => switch = Some(Workspace::Images),
                    egui::Key::Num6 => switch = Some(Workspace::Tools),
                    egui::Key::Num7 => switch = Some(Workspace::Video),
                    _ => return true,
                }
                return false;
            }
            true
        });
        if let Some(w) = switch {
            self.next = Some(w);
        }
        if open {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("iris-open"), true));
        }
        for f in std::mem::take(&mut raw.dropped_files) {
            if let Some(path) = f.path().to_str()
                && let Err(e) = self.open(path, ctx)
            {
                self.notice = Some(e);
            }
        }
        if let Some(result) = self.voice.poll() {
            if result.workspace == self.active {
                if let Some(focus) = result.focus {
                    ctx.memory_mut(|memory| memory.request_focus(focus));
                    raw.events.push(egui::Event::Paste(result.text));
                    self.voice.notice =
                        "Transcript sent to the selected field. A copy is in Voice settings."
                            .into();
                } else if self.active != Workspace::Home {
                    // Canvas editors can hold their own selection without an egui text field.
                    // Let them receive Paste as ordinary editor input.
                    ctx.memory_mut(|memory| {
                        if let Some(id) = memory.focused() {
                            memory.surrender_focus(id);
                        }
                    });
                    raw.events.push(egui::Event::Paste(result.text));
                    self.voice.notice =
                        "Transcript sent to the current editor. A copy is in Voice settings."
                            .into();
                } else {
                    self.voice.panel_open = true;
                    self.voice.notice =
                        "No text field was selected. Copy the transcript from Voice settings."
                            .into();
                }
            } else {
                self.voice.panel_open = true;
                self.voice.notice = "The workspace changed during dictation. Copy the transcript from Voice settings.".into();
            }
        }
        voice::request_repaint(ctx, &self.voice);
        match self.active {
            Workspace::Reader
                if self.source.is_none()
                    && self.media.is_none()
                    && self.image_preview.is_none() =>
            {
                self.reader.raw_input_hook(ctx, raw)
            }
            Workspace::Tools => self.reader.raw_input_hook(ctx, raw),
            Workspace::Writer => self.writer.raw_input_hook(raw),
            Workspace::Sheets => self.sheets.raw_input_hook(raw),
            Workspace::Slides => self.slides.raw_input_hook(raw),
            Workspace::Images => self.images.raw_input_hook(raw),
            _ => {}
        }
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        for event in self.apple.drain() {
            match event {
                fmv_macos_events::Event::Open(paths) => {
                    for p in paths {
                        if let Some(p) = p.to_str()
                            && let Err(e) = self.open(p, ctx)
                        {
                            self.notice = Some(e);
                        }
                    }
                }
                fmv_macos_events::Event::Quit => self.quit_requested = true,
            }
        }
        if let Some(w) = self.next.take() {
            self.activate(w, ctx);
        }
        self.poll_picker(ctx);
        if ctx
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new("iris-open")))
            .unwrap_or(false)
        {
            self.pick(false, frame, ctx);
        }
        self.poll_control(ctx, frame);
        match self.active {
            Workspace::Home => {}
            Workspace::Reader
                if self.source.is_none()
                    && self.media.is_none()
                    && self.image_preview.is_none() =>
            {
                self.reader.logic(ctx, frame)
            }
            Workspace::Reader => {}
            Workspace::Tools => self.reader.logic(ctx, frame),
            Workspace::Writer => self.writer.logic(ctx),
            Workspace::Sheets => self.sheets.logic(ctx),
            Workspace::Slides => self.slides.logic(ctx),
            Workspace::Images => self.images.logic(ctx),
            Workspace::Video => {}
        }
        if self.active == Workspace::Reader {
            if let Some(media) = &mut self.media {
                media.tick(ctx);
            }
            if let Some(image) = &mut self.image_preview {
                image.tick(ctx);
            }
        }
        self.video_editor.tick(ctx);
        if let Some(result) = self.global_dictation.poll() {
            self.voice.last_transcript = result.text;
            self.voice.notice = if result.pasted {
                "Global dictation inserted in the focused app.".into()
            } else {
                "Global dictation copied to the clipboard. Paste it where you want it.".into()
            };
        }
        self.global_dictation.sync_language(&self.voice.language);
        if self
            .reading_memory
            .sync(&mut self.reader, &self.office_previews)
        {
            self.reading_memory_dirty = true;
        }
        if self.reading_memory_dirty && self.reading_memory_saved_at.elapsed().as_secs_f32() >= 2.0
        {
            self.save_preferences();
            self.reading_memory_dirty = false;
            self.reading_memory_saved_at = Instant::now();
        }
        self.sync_editor_recent(true);
        if self.recent_checked_at.elapsed().as_secs() >= 3 {
            self.recent_checked_at = Instant::now();
            if self.prune_missing_recent() {
                // Keep the editor history snapshots aligned so an old entry
                // cannot be promoted as fresh activity on the next frame.
                self.sync_editor_recent(false);
                self.save_preferences();
            }
        }
        if let Some(path) = self.video_editor.take_exported_path()
            && let Some(path) = path.to_str()
        {
            self.recent.retain(|recent| recent != path);
            self.recent.insert(0, path.to_owned());
            self.recent.truncate(16);
            self.save_preferences();
        }
        home_fonts::add_indic_system_fallbacks(ctx);
        // Editor appearance controls feed the shared palette as well as the top-bar toggle.
        let chosen_dark = match self.active {
            Workspace::Reader
                if self.source.is_none()
                    && self.media.is_none()
                    && self.image_preview.is_none() =>
            {
                Some(self.reader.theme == pdfcraft_ui_egui::theme::ThemeKind::Dark)
            }
            Workspace::Tools => Some(self.reader.theme == pdfcraft_ui_egui::theme::ThemeKind::Dark),
            Workspace::Writer => match self.writer.ui.theme {
                wordcraft_ui_egui::theme::Appearance::Light => Some(false),
                wordcraft_ui_egui::theme::Appearance::Dark => Some(true),
                wordcraft_ui_egui::theme::Appearance::System => None,
            },
            Workspace::Sheets if !self.sheets.ui.system_theme => Some(self.sheets.ui.dark),
            Workspace::Slides => {
                Some(self.slides.ui.brightness == deckcraft_ui_egui::theme::Brightness::Dark)
            }
            Workspace::Images => Some(self.images.ui.settings.theme(None).is_dark()),
            _ => None,
        };
        if let Some(dark) = chosen_dark
            && dark != self.dark
        {
            self.set_dark(dark, ctx);
        }
        if self.writer.quit_requested {
            self.writer.quit_requested = false;
            self.quit_requested = true;
        }
        if self.quit_requested && self.dirty_workspaces().is_empty() {
            self.force_quit = true;
        }
        if self.force_quit {
            #[cfg(target_os = "macos")]
            if !self.native_quit_scheduled {
                self.native_quit_scheduled = craft_quit::quit();
            }
            #[cfg(not(target_os = "macos"))]
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        #[cfg(target_os = "macos")]
        craft_quit::on_close_request(ctx);
        self.dialogs.show(frame);
        self.collect_shots(ctx);
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let full_screen = self.active == Workspace::Reader
            && (self.reader.full_screen
                && self.source.is_none()
                && self.media.is_none()
                && self.image_preview.is_none()
                || self.media.as_ref().is_some_and(|media| media.fullscreen)
                || self
                    .image_preview
                    .as_ref()
                    .is_some_and(|image| image.fullscreen));
        if !full_screen {
            self.bar(ui, frame);
            if self.active == Workspace::Reader {
                if self.reader_tabs_ui(ui) {
                    self.pick(false, frame, &ui.ctx().clone());
                }
            }
        }
        ui.push_id(self.active.label(), |ui| match self.active {
            Workspace::Home => self.home(ui, frame),
            Workspace::Reader if self.source.is_some() => {
                if let Some(source) = &mut self.source {
                    source_view::show(ui, source, self.dark);
                }
            }
            Workspace::Reader if self.media.is_some() => {
                if let Some(media) = &mut self.media {
                    media.show(ui, self.dark);
                }
            }
            Workspace::Reader if self.image_preview.is_some() => {
                if let Some(image) = &mut self.image_preview {
                    image.show(ui, self.dark);
                }
            }
            Workspace::Reader if self.reader.active.is_none() => self.reader_landing(ui, frame),
            Workspace::Reader => self.reader.ui(ui, frame),
            Workspace::Tools => self.reader.ui(ui, frame),
            Workspace::Writer => self.writer.ui(ui),
            Workspace::Sheets => self.sheets.ui(ui),
            Workspace::Slides => self.slides.ui(ui),
            Workspace::Images => {
                if let Some(error) = &self.images_error {
                    ui.heading("Images library unavailable");
                    ui.label(error);
                } else {
                    self.images.ui(ui);
                }
            }
            Workspace::Video => self.video_editor.show(ui, frame, self.dark),
        });
        let requested = match self.active {
            Workspace::Writer => self.writer.external_url_requested.take(),
            Workspace::Sheets => self.sheets.external_url_requested.take(),
            Workspace::Slides => self.slides.external_url_requested.take(),
            _ => None,
        };
        if let Some(address) = requested {
            self.queue_external_link(address);
        }
        if let Some(address) = self.external_link.clone() {
            let email = address
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("mailto:"));
            let action = if email {
                "Open email app"
            } else {
                "Open in browser"
            };
            let mut decision = None;
            let modal =
                egui::Modal::new(egui::Id::new("polymath-external-link")).show(ui.ctx(), |ui| {
                    ui.set_width(440.0);
                    ui.heading("Open document link?");
                    ui.label(if email {
                        "This link will open your default email app."
                    } else {
                        "This link will open in your default browser."
                    });
                    if let Some(host) = pdfcraft_engine::links::display_host(&address) {
                        ui.label(egui::RichText::new(host.ascii).strong());
                        if host.mixed_scripts {
                            ui.colored_label(
                                egui::Color32::from_rgb(185, 55, 55),
                                "This address mixes letters from different alphabets.",
                            );
                        }
                    }
                    egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(&address).monospace().small())
                                    .wrap()
                                    .selectable(true),
                            );
                        });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            decision = Some(false);
                        }
                        if ui.button("Copy address").clicked() {
                            ui.ctx().copy_text(address.clone());
                            decision = Some(false);
                        }
                        if ui.button(action).clicked() {
                            decision = Some(true);
                        }
                    });
                });
            if decision.is_none() && modal.should_close() {
                decision = Some(false);
            }
            if let Some(open) = decision {
                self.external_link = None;
                if open {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(address));
                }
            }
        }
        if self.quit_requested && !self.force_quit {
            egui::Modal::new(egui::Id::new("iris-quit")).show(ui.ctx(), |ui| {
                for font in ui.style_mut().text_styles.values_mut() {
                    font.family = egui::FontFamily::Proportional;
                }
                ui.set_width(420.0);
                ui.heading("Save your changes before leaving Polymath");
                ui.label("These workspaces have unsaved changes:");
                for w in self.dirty_workspaces() {
                    if ui.button(format!("Review {}", w.label())).clicked() {
                        self.quit_requested = false;
                        self.next = Some(w);
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Keep working").clicked() {
                        self.quit_requested = false;
                    }
                    if ui.button("Quit without saving").clicked() {
                        self.force_quit = true;
                        ui.ctx().request_repaint();
                    }
                });
            });
        }
        if let Some(message) = self.notice.clone() {
            let colors = ui_kit::chrome(self.dark);
            let missing = message.strip_prefix("File not found: ");
            let modal = egui::Modal::new(egui::Id::new("polymath-notice")).show(ui.ctx(), |ui| {
                ui.set_width(420.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(if missing.is_some() { "File unavailable" } else { "Couldn't complete the action" })
                        .size(18.0).strong().color(colors.ink));
                    ui.add_space(8.0);
                    if let Some(path) = missing {
                        ui.label(egui::RichText::new("The file may have moved or been deleted. It has been removed from Recent files.").size(13.0).color(colors.muted));
                        ui.add_space(12.0);
                        egui::Frame::NONE.fill(colors.canvas).stroke(egui::Stroke::new(1.0, colors.line))
                            .corner_radius(9).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(path).size(12.0).color(colors.ink)).wrap().selectable(true));
                            });
                    } else {
                        ui.add(egui::Label::new(egui::RichText::new(&message).size(13.0).color(colors.muted)).wrap());
                    }
                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Dismiss").color(colors.surface)).fill(colors.ink).stroke(egui::Stroke::NONE))
                            .on_hover_text("Close this notice").clicked() {
                            self.notice = None;
                        }
                    });
                });
            });
            if modal.should_close() {
                self.notice = None;
            }
        }
        self.issue_shots(ui.ctx());
        self.dialogs.show(frame);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "Polymath — {}",
                self.active.label()
            )));
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_preferences();
        self.reader.shutdown_recovery();
        if let Err(error) = self.images.session.close_library() {
            eprintln!("Polymath Images library could not be saved: {error}");
        }
        let _ = self.images.shutdown(std::time::Duration::from_secs(2));
    }
}
fn main() -> eframe::Result {
    let mut control_path = None;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--control" => control_path = args.next().map(std::path::PathBuf::from),
            "--version" => {
                println!("Polymath {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" => {
                println!(
                    "Polymath [--control <new-private-endpoint.json>] [files…]\nOne local window for PDFs, documents, spreadsheets, presentations and images."
                );
                return Ok(());
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {arg}");
                return Ok(());
            }
            _ => files.push(arg),
        }
    }
    #[cfg(target_os = "macos")]
    let (_registration, inbox) = fmv_macos_events::Registration::install();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Polymath")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([980.0, 650.0])
        .with_app_id("local.polymath.app")
        .with_drag_and_drop(true);
    if let Ok(icon) =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/polymath-1024.png"))
    {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Polymath",
        options,
        Box::new(move |cc| {
            #[cfg(target_os = "macos")]
            {
                let wake = cc.egui_ctx.clone();
                inbox.set_wake(move || wake.request_repaint());
            }
            let mut app = Polymath::new(
                &cc.egui_ctx,
                #[cfg(target_os = "macos")]
                inbox,
            );
            if let Some(path) = control_path {
                match control::start(path, cc.egui_ctx.clone()) {
                    Ok(s) => app.server = Some(s),
                    Err(e) => app.notice = Some(format!("Agent control was not started: {e}")),
                }
            }
            for path in files {
                if let Err(e) = app.open(&path, &cc.egui_ctx) {
                    app.notice = Some(e);
                }
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;
    fn app(ctx: &egui::Context) -> Polymath {
        Polymath::new(
            ctx,
            #[cfg(target_os = "macos")]
            Default::default(),
        )
    }
    #[test]
    fn document_link_requests_accept_web_addresses_and_reject_local_files() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.queue_external_link("https://example.org/guide".into());
        assert_eq!(
            app.external_link.as_deref(),
            Some("https://example.org/guide")
        );
        app.external_link = None;
        app.queue_external_link("file:///private/notes.txt".into());
        assert!(app.external_link.is_none());
        assert!(app.notice.is_some());
    }
    #[test]
    fn reader_first_open_keeps_switchable_and_closable_file_tabs() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let dir = std::env::temp_dir().join(format!(
            "polymath-reader-tabs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let note = dir.join("notes.md");
        let table = dir.join("budget.csv");
        let pdf = dir.join("book.pdf");
        std::fs::write(&note, "# Notes\nKeep this tab open.").expect("markdown");
        std::fs::write(&table, "Name,Amount\nPaper,12\n").expect("csv");
        std::fs::write(&pdf, b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 300 400] >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF").expect("pdf");
        let note = note.to_string_lossy().into_owned();
        let table = table.to_string_lossy().into_owned();
        let pdf = pdf.to_string_lossy().into_owned();
        for path in [&note, &table, &pdf] {
            app.open(path, &ctx).expect("open in Reader");
            assert_eq!(app.active, Workspace::Reader);
            assert_eq!(app.current_reader_path().as_deref(), Some(path.as_str()));
        }
        assert_eq!(app.reader_tabs().len(), 3);
        assert_eq!(app.reader_tabs().iter().filter(|tab| tab.active).count(), 1);
        assert!(app.select_reader_path(&note, &ctx));
        assert_eq!(app.current_reader_path().as_deref(), Some(note.as_str()));
        assert!(app.select_reader_path(&table, &ctx));
        assert_eq!(app.current_reader_path().as_deref(), Some(table.as_str()));
        let note_id = app.find_reader_tab_by_path(&note).expect("note tab");
        assert!(app.close_reader_tab(note_id, &ctx));
        assert_eq!(app.reader_tabs().len(), 2);
        assert!(!app.select_reader_path(&note, &ctx));
        assert!(app.close_reader_item(&ctx));
        assert_eq!(app.reader_tabs().len(), 1);
        assert_eq!(app.current_reader_path().as_deref(), Some(pdf.as_str()));
        std::fs::remove_dir_all(dir).expect("remove scratch directory");
    }
    #[test]
    fn missing_recent_file_is_pruned_on_open() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let path = format!("/missing/polymath-{}-not-here.pdf", std::process::id());
        app.recent.push(path.clone());
        assert!(app.open(&path, &ctx).is_err());
        assert!(!app.recent.contains(&path));
    }
    #[test]
    fn missing_recent_click_silently_prunes_every_history() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let missing = format!("/missing/polymath-{}-moved.pdf", std::process::id());
        let existing =
            std::env::temp_dir().join(format!("polymath-recent-existing-{}", std::process::id()));
        std::fs::write(&existing, b"file").expect("create existing file");
        let existing = existing.to_string_lossy().into_owned();
        app.recent.extend([missing.clone(), existing.clone()]);
        app.reader.recent.push(pdfcraft_ui_egui::RecentFile {
            name: "moved.pdf".into(),
            path: missing.clone(),
            pages: 1,
            size: 0,
        });
        app.writer.ui.recent.push(missing.clone());
        app.sheets.ui.recent.push(missing.clone());
        app.slides.ui.recent.push(missing.clone());

        assert!(app.open_recent_or_prune(&missing, &ctx).is_ok());
        assert_eq!(app.recent, [existing.clone()]);
        assert!(app.reader.recent.is_empty());
        assert!(app.writer.ui.recent.is_empty());
        assert!(app.sheets.ui.recent.is_empty());
        assert!(app.slides.ui.recent.is_empty());
        assert!(app.notice.is_none());
        std::fs::remove_file(existing).expect("remove existing file");
    }
    #[test]
    fn reader_selects_remaining_preview_after_confirmed_pdf_close() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let dir = std::env::temp_dir().join(format!(
            "polymath-reader-close-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let note = dir.join("remaining.txt");
        std::fs::write(&note, "Keep reading").expect("text fixture");
        let note = note.to_string_lossy().into_owned();
        app.open(&note, &ctx).expect("open text");
        let pdf = b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 300 400] >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF";
        app.stash_reader_preview(&ctx);
        app.reader
            .open_bytes("temporary.pdf", None, pdf.to_vec())
            .expect("open PDF");
        app.sync_reader_tabs();
        assert!(matches!(
            app.active_reader_tab(),
            Some(reader_tabs::TabId::Pdf(_))
        ));
        let index = app.reader.active.expect("active PDF");
        app.reader.close_tab(index); // The save prompt resolves through this same Reader close path.
        app.ensure_reader_tab(&ctx);
        assert_eq!(app.current_reader_path().as_deref(), Some(note.as_str()));
        assert_eq!(app.reader_tabs().len(), 1);
        std::fs::remove_dir_all(dir).expect("remove scratch directory");
    }
    #[test]
    fn editor_histories_and_new_saves_appear_in_home_recent() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let dir = std::env::temp_dir().join(format!("polymath-home-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create scratch folder");
        let names = [
            "already-open.pdf",
            "draft.docx",
            "budget.csv",
            "slides.pptx",
            "new.docx",
        ];
        let paths = names.map(|name| dir.join(name).to_string_lossy().into_owned());
        for path in &paths {
            std::fs::write(path, b"test").expect("create synthetic file");
        }
        app.recent.push(paths[0].clone());
        app.reader.recent.push(pdfcraft_ui_egui::RecentFile {
            name: names[0].into(),
            path: paths[0].clone(),
            pages: 1,
            size: 4,
        });
        app.writer.ui.recent.push(paths[1].clone());
        app.sheets.ui.recent.push(paths[2].clone());
        app.slides.ui.recent.push(paths[3].clone());
        app.sync_editor_recent(false);
        assert_eq!(
            app.recent,
            paths[..4],
            "the existing Home order stays first and each editor adds its files"
        );

        app.writer.ui.recent.insert(0, paths[4].clone());
        app.sync_editor_recent(true);
        assert_eq!(
            app.recent.first(),
            Some(&paths[4]),
            "a newly saved document reaches the top of Home"
        );
        app.sync_editor_recent(true);
        assert_eq!(
            app.recent.iter().filter(|path| *path == &paths[4]).count(),
            1
        );
        std::fs::remove_dir_all(dir).expect("remove synthetic files");
    }
    #[test]
    fn workspace_switches_preserve_three_documents_and_undo() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        assert!(
            app.writer
                .execute("text.insert", json!({"text":"Keep this draft."}))
                .is_ok()
        );
        app.sheets.session.new_workbook();
        assert!(
            app.sheets
                .run("cell.set", json!({"cell":"A1","input":"42"}))
                .is_ok()
        );
        assert!(app.slides.run("file.new", json!({})).is_ok());
        assert!(app.slides.run("slide.new", json!({})).is_ok());
        let words = app.writer.session.word_count();
        let undo = app.writer.session.undo_depth();
        let sheet = app
            .sheets
            .session
            .run("cell.get", json!({"cell":"A1"}))
            .expect("read cell");
        let slides = app
            .slides
            .session
            .execute("document.inspect", &json!({}))
            .expect("inspect deck");
        for _ in 0..3 {
            for w in Workspace::ALL {
                app.activate(w, &ctx);
            }
        }
        assert_eq!(app.writer.session.word_count(), words);
        assert_eq!(app.writer.session.undo_depth(), undo);
        assert_eq!(
            app.sheets
                .session
                .run("cell.get", json!({"cell":"A1"}))
                .expect("read retained cell"),
            sheet
        );
        assert_eq!(
            app.slides
                .session
                .execute("document.inspect", &json!({}))
                .expect("retained deck"),
            slides
        );
        assert!(app.dirty_workspaces().contains(&Workspace::Writer));
        assert!(app.dirty_workspaces().contains(&Workspace::Sheets));
        assert!(app.dirty_workspaces().contains(&Workspace::Slides));
        assert!(app.writer.session.undo());
        assert_eq!(app.writer.session.word_count(), 0);
    }
    #[test]
    fn native_close_is_guarded_when_dirty_editor_is_inactive() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.writer
            .execute("text.insert", json!({"text":"Unsaved"}))
            .expect("insert");
        assert_eq!(app.active, Workspace::Home);
        let mut raw = egui::RawInput::default();
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        eframe::App::raw_input_hook(&mut app, &ctx, &mut raw);
        assert!(app.quit_requested);
        assert!(!app.force_quit);
        assert!(
            raw.viewports
                .values()
                .all(|v| !v.events.contains(&egui::ViewportEvent::Close))
        );
        assert_eq!(app.writer.session.word_count(), 1);
    }
    #[test]
    fn all_editor_fonts_survive_repeated_real_ui_passes() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1200.0, 800.0))
            .build_eframe(|cc| app(&cc.egui_ctx));
        harness.run_steps(4);
        for w in [
            Workspace::Writer,
            Workspace::Sheets,
            Workspace::Slides,
            Workspace::Reader,
            Workspace::Home,
            Workspace::Writer,
            Workspace::Sheets,
            Workspace::Home,
        ] {
            harness.state_mut().next = Some(w);
            harness.run_steps(5);
            assert_eq!(harness.state().active, w);
        }
    }
    #[test]
    fn settings_gear_collects_app_preferences() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1200.0, 800.0))
            .build_eframe(|cc| app(&cc.egui_ctx));
        harness.run_steps(4);
        harness
            .get_by_role_and_label(Role::Button, "Settings")
            .click();
        harness.run_steps(3);
        assert!(harness.state().voice.panel_open);
        harness.get_by_role_and_label(Role::Button, "Voice").click();
        harness.run_steps(2);
        assert_eq!(harness.state().settings_page, settings::Page::Voice);
        harness
            .get_by_role_and_label(Role::Button, "Files & privacy")
            .click();
        harness.run_steps(2);
        assert_eq!(harness.state().settings_page, settings::Page::Files);
    }

    #[test]
    fn toolbar_mic_appears_only_in_text_editing_workspaces() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1200.0, 800.0))
            .build_eframe(|cc| app(&cc.egui_ctx));
        for (workspace, expected) in [
            (Workspace::Home, false),
            (Workspace::Reader, false),
            (Workspace::Writer, true),
            (Workspace::Sheets, true),
            (Workspace::Slides, true),
            (Workspace::Tools, true),
            (Workspace::Images, false),
            (Workspace::Video, false),
        ] {
            harness.state_mut().next = Some(workspace);
            harness.run_steps(5);
            assert_eq!(harness.state().active, workspace);
            assert_eq!(
                harness
                    .query_by_role_and_label(Role::Button, "Dictate into the selected text field")
                    .is_some(),
                expected,
                "mic visibility in {workspace:?}"
            );
        }
    }
}
