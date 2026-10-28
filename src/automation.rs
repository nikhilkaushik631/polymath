use crate::{
    Polymath, Shot, Workspace,
    control::{self, Request},
    media_view,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
impl Polymath {
    pub(crate) fn poll_control(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        while let Some(req) = self
            .server
            .as_ref()
            .and_then(|s| s.requests.try_recv().ok())
        {
            self.dispatch(req, ctx, frame);
        }
        self.pdf_pending.retain(|(rx,reply,started)|{
            match rx.try_recv() {
                Ok(result)=>{let _=reply.send(match result {Ok(v)=>control::ok(v),Err(e)=>control::error(e)});false},
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{let _=reply.send(control::error("Reader control closed"));false},
                _ if started.elapsed()>Duration::from_secs(12)=>{let _=reply.send(control::error("Reader did not finish; activate Reader, inspect state, and retry read-only requests"));false},
                _=>true,
            }
        });
        if !self.pdf_pending.is_empty() || !self.shots.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
    fn dispatch(&mut self, req: Request, ctx: &egui::Context, frame: &eframe::Frame) {
        let p = &req.params;
        let result = match req.method.as_str() {
            "app.inspect" => control::ok(self.summary()),
            "app.workspace" => match p
                .get("workspace")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing workspace".to_string())
                .and_then(Workspace::parse)
            {
                Ok(w) => {
                    self.activate(w, ctx);
                    control::ok(self.summary())
                }
                Err(e) => control::error(e),
            },
            "app.open" => match p.get("path").and_then(Value::as_str) {
                Some(path) => match if p.get("edit").and_then(Value::as_bool) == Some(true) {
                    self.open_for_edit(path, ctx)
                } else {
                    self.open(path, ctx)
                } {
                    Ok(v) => control::ok(v),
                    Err(e) => control::error(e),
                },
                None => control::error("missing path"),
            },
            "app.preview" => match p.get("path").and_then(Value::as_str) {
                Some(path) => match self.open_in_reader(path, ctx) {
                    Ok(v) => control::ok(v),
                    Err(e) => control::error(e),
                },
                None => control::error("missing path"),
            },
            "app.video.inspect" => control::ok(self.video_editor.inspect()),
            "app.video.add" => match p.get("path").and_then(Value::as_str) {
                Some(path) if crate::workspace::is_video_path(std::path::Path::new(path)) => {
                    match self.open_for_edit(path, ctx) {
                        Ok(value) => control::ok(value),
                        Err(error) => control::error(error),
                    }
                }
                _ => control::error("Choose a local video path"),
            },
            "app.video.trim" => {
                let index = p
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok());
                let start = p.get("start").and_then(Value::as_f64);
                let end = p.get("end").and_then(Value::as_f64);
                match (index, start, end) {
                    (Some(index), Some(start), Some(end)) => {
                        match self.video_editor.set_clip_trim(index, start, end) {
                            Ok(()) => control::ok(self.video_editor.inspect()),
                            Err(error) => control::error(error),
                        }
                    }
                    _ => control::error("Provide clip index, start and end seconds"),
                }
            }
            "app.video.move" => {
                let from = p
                    .get("from")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok());
                let to = p
                    .get("to")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok());
                match (from, to) {
                    (Some(from), Some(to)) => match self.video_editor.move_clip(from, to) {
                        Ok(()) => control::ok(self.video_editor.inspect()),
                        Err(error) => control::error(error),
                    },
                    _ => control::error("Provide source and destination clip indexes"),
                }
            }
            "app.video.audio" => match p.get("path").and_then(Value::as_str) {
                Some(path) => match self.video_editor.set_audio_path(path.into(), ctx) {
                    Ok(()) => control::ok(self.video_editor.inspect()),
                    Err(error) => control::error(error),
                },
                None => control::error("Choose a local audio path"),
            },
            "app.video.audio.options" => {
                let start = p.get("start").and_then(Value::as_f64);
                let gain = p.get("gain").and_then(Value::as_f64);
                match (start, gain) {
                    (Some(start), Some(gain)) => {
                        match self.video_editor.set_audio_options(start, gain as f32) {
                            Ok(()) => control::ok(self.video_editor.inspect()),
                            Err(error) => control::error(error),
                        }
                    }
                    _ => control::error("Provide audio timeline start and gain"),
                }
            }
            "app.video.mute" => match p.get("original").and_then(Value::as_bool) {
                Some(mute) => {
                    self.video_editor.set_mute_original(mute);
                    control::ok(self.video_editor.inspect())
                }
                None => control::error("original must be a boolean"),
            },
            "app.video.quality" => {
                let quality = match p.get("preset").and_then(Value::as_str) {
                    Some("original") => Some(crate::video_pipeline::Quality::Original),
                    Some("high") => Some(crate::video_pipeline::Quality::High),
                    Some("balanced") => Some(crate::video_pipeline::Quality::Balanced),
                    Some("small") => Some(crate::video_pipeline::Quality::Small),
                    _ => None,
                };
                if let Some(quality) = quality {
                    self.video_editor.set_quality(quality);
                    control::ok(self.video_editor.inspect())
                } else {
                    control::error("preset must be original, high, balanced or small")
                }
            }
            "app.video.output" => match p.get("path").and_then(Value::as_str) {
                Some(path) => match self.video_editor.set_output_path(path.into()) {
                    Ok(()) => control::ok(self.video_editor.inspect()),
                    Err(error) => control::error(error),
                },
                None => control::error("Choose an output path"),
            },
            "app.video.export" => match self.video_editor.start_export() {
                Ok(()) => control::ok(self.video_editor.inspect()),
                Err(error) => control::error(error),
            },
            "app.image.fullscreen" => match (
                &mut self.image_preview,
                self.active == Workspace::Reader,
                p.get("enabled").and_then(Value::as_bool),
            ) {
                (Some(image), true, Some(enabled)) => {
                    image.set_fullscreen(ctx, enabled);
                    control::ok(image.inspect())
                }
                (_, _, None) => control::error("enabled must be a boolean"),
                _ => control::error("Open an image in Reader first"),
            },
            "app.reader.tabs" => control::ok(self.reader_tabs_json()),
            "app.source.goto" => match (
                self.active == Workspace::Reader,
                self.source.as_mut(),
                p.get("line")
                    .and_then(Value::as_u64)
                    .and_then(|line| usize::try_from(line).ok()),
            ) {
                (true, Some(source), Some(line)) => match source.go_to_line(line) {
                    Ok(()) => {
                        ctx.request_repaint();
                        control::ok(
                            json!({"path":source.path,"line":line,"lines":source.line_count()}),
                        )
                    }
                    Err(error) => control::error(error),
                },
                (true, Some(_), None) => control::error("line must be a one-based integer"),
                _ => control::error("Open a source file in Reader first"),
            },
            "app.reader.select" => match p
                .get("id")
                .and_then(Value::as_str)
                .and_then(crate::reader_tabs::TabId::parse)
                .or_else(|| {
                    p.get("path")
                        .and_then(Value::as_str)
                        .and_then(|path| self.find_reader_tab_by_path(path))
                }) {
                Some(id) if self.select_reader_tab(id, ctx) => control::ok(self.reader_tabs_json()),
                _ => control::error("Choose an open Reader tab by id"),
            },
            "app.reader.close" => {
                let target = p
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(crate::reader_tabs::TabId::parse)
                    .or_else(|| {
                        p.get("path")
                            .and_then(Value::as_str)
                            .and_then(|path| self.find_reader_tab_by_path(path))
                    });
                let closed = if let Some(id) = target {
                    self.close_reader_tab(id, ctx)
                } else if p.get("id").is_some() || p.get("path").is_some() {
                    false
                } else {
                    self.close_reader_item(ctx)
                };
                if closed {
                    control::ok(json!({"closeRequested":true,"workspace":self.active}))
                } else {
                    control::error("Open a file in Reader first")
                }
            }
            "app.media.inspect" => match &self.media {
                Some(media) if self.active == Workspace::Reader => control::ok(media.inspect()),
                _ => control::error("Open an audio or video file in Reader first"),
            },
            "app.media.queue" => match &self.media {
                Some(media) if self.active == Workspace::Reader => {
                    let offset = p
                        .get("offset")
                        .and_then(Value::as_u64)
                        .and_then(|value| usize::try_from(value).ok())
                        .unwrap_or(0);
                    let limit = p
                        .get("limit")
                        .and_then(Value::as_u64)
                        .and_then(|value| usize::try_from(value).ok())
                        .unwrap_or(50);
                    control::ok(media.queue_page(offset, limit))
                }
                _ => control::error("Open an audio or video file in Reader first"),
            },
            "app.media.play"
            | "app.media.pause"
            | "app.media.seek"
            | "app.media.next"
            | "app.media.previous"
            | "app.media.sort"
            | "app.media.volume"
            | "app.media.fullscreen" => {
                if self.active != Workspace::Reader {
                    control::error("Open an audio or video file in Reader first")
                } else if let Some(media) = &mut self.media {
                    let action = match req.method.as_str() {
                        "app.media.play" => media.play(),
                        "app.media.pause" => {
                            media.pause();
                            Ok(())
                        }
                        "app.media.next" => media.adjacent(1),
                        "app.media.previous" => media.adjacent(-1),
                        "app.media.sort" => p
                            .get("by")
                            .and_then(Value::as_str)
                            .and_then(media_view::MediaView::sort_from_label)
                            .ok_or_else(|| "by must be name, artist or album".to_owned())
                            .map(|sort| media.set_sort(sort)),
                        "app.media.volume" => p
                            .get("value")
                            .and_then(Value::as_f64)
                            .ok_or_else(|| "value must be a number from 0 to 1".to_owned())
                            .and_then(|value| media.set_volume(value as f32)),
                        "app.media.fullscreen" => p
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .ok_or_else(|| "enabled must be a boolean".to_owned())
                            .and_then(|enabled| media.set_fullscreen(ctx, enabled)),
                        _ => p
                            .get("seconds")
                            .and_then(Value::as_f64)
                            .ok_or_else(|| "seconds must be a number".to_owned())
                            .and_then(|seconds| media.seek(seconds)),
                    };
                    match action {
                        Ok(()) => {
                            ctx.request_repaint();
                            control::ok(media.inspect())
                        }
                        Err(error) => control::error(error),
                    }
                } else {
                    control::error("Open an audio or video file in Reader first")
                }
            }
            "app.reader.favorites" => control::ok(
                json!({"files":self.reading_memory.favorites().iter().map(|path| json!({"path":path,"page":self.reading_memory.page(path).map(|page| page + 1)})).collect::<Vec<_>>()}),
            ),
            "app.reader.favorite" => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| self.current_reader_path());
                match (path, p.get("value").and_then(Value::as_bool)) {
                    (Some(path), Some(value)) if std::path::Path::new(&path).is_file() => {
                        self.reading_memory.set_favorite(&path, value);
                        self.save_preferences();
                        control::ok(json!({"path":path,"favorite":value}))
                    }
                    (Some(_), Some(_)) => {
                        control::error("Favorite must be a local file that exists")
                    }
                    (None, _) => control::error("Open a file in Reader or provide a path"),
                    (_, None) => control::error("value must be a boolean"),
                }
            }
            "app.theme" => match p.get("dark").and_then(Value::as_bool) {
                Some(dark) => {
                    self.set_dark(dark, ctx);
                    control::ok(self.summary())
                }
                None => control::error("dark must be a boolean"),
            },
            "app.settings" => {
                if let Some(page) = p.get("page").and_then(Value::as_str) {
                    match crate::settings::Page::parse(page) {
                        Some(page) => self.settings_page = page,
                        None => {
                            let _ = req
                                .reply
                                .send(control::error("page must be appearance, voice, or files"));
                            return;
                        }
                    }
                }
                self.voice.panel_open = p.get("visible").and_then(Value::as_bool).unwrap_or(true);
                control::ok(
                    json!({"visible":self.voice.panel_open,"page":format!("{:?}",self.settings_page).to_lowercase()}),
                )
            }
            "app.quit" => {
                self.quit_requested = true;
                control::ok(json!({"requested":true,"dirtyWorkspaces":self.dirty_workspaces()}))
            }
            "app.quit.cancel" => {
                self.quit_requested = false;
                control::ok(json!({"cancelled":true}))
            }
            "app.folders.choose" => {
                self.pick(true, frame, ctx);
                control::ok(json!({"waitingForUser":true}))
            }
            "app.folders.list" => {
                control::ok(json!({"folders":self.folders,"listings":self.listing.listings}))
            }
            "app.folders.remove" => match p.get("path").and_then(Value::as_str) {
                Some(path) => {
                    self.folders.retain(|p| p != path);
                    self.listing.forget(path);
                    control::ok(json!({"folders":self.folders}))
                }
                None => control::error("missing path"),
            },
            "ui.screenshot" => {
                if !p
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| std::path::Path::new(path).is_absolute())
                {
                    let _ = req
                        .reply
                        .send(control::error("Screenshot needs an absolute output path"));
                    return;
                }
                self.shots.push(Shot {
                    reply: req.reply,
                    path: p.get("path").and_then(Value::as_str).map(str::to_string),
                    after: 8,
                    started: Instant::now(),
                    issued: false,
                });
                ctx.request_repaint();
                return;
            }
            _ => {
                if let Some(name) = p.get("workspace").and_then(Value::as_str) {
                    match Workspace::parse(name) {
                        Ok(w) => self.activate(w, ctx),
                        Err(e) => {
                            let _ = req.reply.send(control::error(e));
                            return;
                        }
                    }
                }
                match self.active {
                    Workspace::Home => control::error(
                        "Choose an editor with app.workspace before sending editor commands",
                    ),
                    Workspace::Writer => {
                        let (request, _) =
                            wordcraft_ui_egui::control::ControlRequest::new(&req.method, p.clone());
                        match wordcraft_ui_egui::control::handle(&mut self.writer, ctx, &request) {
                            wordcraft_ui_egui::control::Outcome::Done(v) => v,
                            _ => control::error("Use ui.screenshot on Polymath"),
                        }
                    }
                    Workspace::Sheets => match gridcraft_ui_egui::control::handle(
                        &mut self.sheets,
                        ctx,
                        &req.method,
                        p,
                    ) {
                        gridcraft_ui_egui::control::Outcome::Done(v) => v,
                        _ => control::error("Use ui.screenshot on Polymath"),
                    },
                    Workspace::Slides => {
                        let (request, _) =
                            deckcraft_ui_egui::control::ControlRequest::new(&req.method, p.clone());
                        match deckcraft_ui_egui::control::handle(&mut self.slides, ctx, &request) {
                            deckcraft_ui_egui::control::Outcome::Done(v) => v,
                            _ => control::error("Use ui.screenshot on Polymath"),
                        }
                    }
                    Workspace::Reader | Workspace::Tools => {
                        if self.active == Workspace::Reader
                            && let Some(image) = &self.image_preview
                        {
                            if matches!(req.method.as_str(), "document.inspect" | "ui.inspect") {
                                let _ = req.reply.send(control::ok(image.inspect()));
                                return;
                            }
                            let _ = req.reply.send(control::error("Images are read-only in Reader. Choose Edit in Images to change the original."));
                            return;
                        }
                        if self.active == Workspace::Reader
                            && let Some(media) = &self.media
                        {
                            if matches!(req.method.as_str(), "document.inspect" | "ui.inspect") {
                                let _ = req.reply.send(control::ok(media.inspect()));
                                return;
                            }
                            let _ = req
                                .reply
                                .send(control::error("Media playback is read-only in Reader."));
                            return;
                        }
                        if self.active == Workspace::Reader
                            && let Some(path) = self.active_office_preview_path()
                            && matches!(req.method.as_str(), "document.inspect" | "ui.inspect")
                        {
                            let _ = req.reply.send(control::ok(json!({"kind":"officePreview","path":path,"readOnly":true,"pdf":self.reader.session.docs().len()})));
                            return;
                        }
                        if self.active == Workspace::Reader
                            && self.active_office_preview_path().is_some()
                            && !matches!(req.method.as_str(), "ui.state" | "ui.commands")
                        {
                            let _ = req.reply.send(control::error("Office previews are read-only. Open the original in Writer, Sheets or Slides to edit it."));
                            return;
                        }
                        if self.active == Workspace::Reader
                            && let Some(source) = &self.source
                        {
                            if req.method == "document.inspect" || req.method == "ui.inspect" {
                                let _ = req.reply.send(control::ok(json!({"kind":"source","path":source.path,"language":source.language,"lines":source.line_count(),"text":source.text})));
                                return;
                            }
                            let _ = req.reply.send(control::error("Source files are read-only in Reader. Open the file in an editor to change it."));
                            return;
                        }
                        if req.method == "engine.execute"
                            && p.get("params").is_some_and(|v| {
                                !v.is_null() && v.as_object().is_none_or(|o| !o.is_empty())
                            })
                        {
                            let _ = req.reply.send(control::error("Reader UI commands take an ID only. Use ui.set for view options or the Reader CLI for parameterized PDF operations."));
                            return;
                        }
                        let (method, params) = match req.method.as_str() {
                            "document.inspect" => ("ui.state", p.clone()),
                            "engine.commands" => ("ui.commands", p.clone()),
                            "engine.execute" => ("ui.command", json!({"id":p["command"]})),
                            _ => (req.method.as_str(), p.clone()),
                        };
                        let rx = self.pdf_control.send(method, params);
                        self.pdf_pending.push((rx, req.reply, Instant::now()));
                        return;
                    }
                    Workspace::Images => {
                        if req.method == "document.inspect" {
                            let ui = lightcraft_ui_egui::control::inspect(&self.images, ctx);
                            let photo = self
                                .images
                                .session
                                .active()
                                .and_then(|_| self.images.run("photo.inspect", json!({})).ok());
                            control::ok(json!({"ui":ui,"photo":photo}))
                        } else {
                            let (request, _) = lightcraft_ui_egui::control::ControlRequest::new(
                                &req.method,
                                p.clone(),
                            );
                            match lightcraft_ui_egui::control::handle(
                                &mut self.images,
                                ctx,
                                &request,
                            ) {
                                lightcraft_ui_egui::control::Outcome::Done(value) => value,
                                lightcraft_ui_egui::control::Outcome::Screenshot { .. } => {
                                    control::error("Use ui.screenshot on Polymath")
                                }
                            }
                        }
                    }
                    Workspace::Video => {
                        if matches!(req.method.as_str(), "document.inspect" | "ui.inspect") {
                            control::ok(self.video_editor.inspect())
                        } else {
                            control::error(
                                "Use the Video workspace timeline to trim, arrange and export clips",
                            )
                        }
                    }
                }
            }
        };
        let _ = req.reply.send(result);
        ctx.request_repaint();
    }
    pub(crate) fn issue_shots(&mut self, ctx: &egui::Context) {
        for shot in &mut self.shots {
            if !shot.issued {
                if shot.after > 0 {
                    shot.after -= 1;
                    ctx.request_repaint();
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                        "iris-suite-shot",
                    )));
                    shot.issued = true;
                }
            }
        }
    }
    pub(crate) fn collect_shots(&mut self, ctx: &egui::Context) {
        let captured = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot {
                    image, user_data, ..
                } if user_data
                    .data
                    .as_ref()
                    .and_then(|d| d.downcast_ref::<&str>())
                    == Some(&"iris-suite-shot") =>
                {
                    Some(image.clone())
                }
                _ => None,
            })
        });
        self.shots.retain(|shot| {
            if let Some(image) = &captured
                && shot.issued
            {
                let _ = shot.reply.send(wordcraft_ui_egui::control::save_screenshot(
                    image,
                    shot.path.as_deref(),
                ));
                false
            } else if shot.started.elapsed() > Duration::from_secs(15) {
                let _ = shot.reply.send(control::error(
                    "Screenshot timed out; bring Polymath to the front",
                ));
                false
            } else {
                true
            }
        });
    }
}
