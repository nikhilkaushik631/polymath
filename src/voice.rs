//! Local speech input and PDF narration. No network service is started.
use crate::Workspace;
use pdfcraft_engine::export::{ExportSource, Exporter};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

const LANGUAGES: &[(&str, &str)] = &[
    ("Detect language", "auto"),
    ("English", "en"),
    ("Hindi", "hi"),
    ("Assamese", "as"),
    ("Bengali", "bn"),
    ("Gujarati", "gu"),
    ("Marathi", "mr"),
    ("Tamil", "ta"),
    ("Telugu", "te"),
    ("Kannada", "kn"),
    ("Malayalam", "ml"),
    ("Nepali", "ne"),
    ("Punjabi", "pa"),
    ("Sanskrit", "sa"),
    ("Sindhi", "sd"),
    ("Urdu", "ur"),
    ("Spanish", "es"),
    ("French", "fr"),
    ("German", "de"),
    ("Italian", "it"),
    ("Portuguese", "pt"),
    ("Arabic", "ar"),
    ("Russian", "ru"),
    ("Japanese", "ja"),
    ("Korean", "ko"),
    ("Chinese", "zh"),
];

struct Dictation {
    child: Child,
    workspace: Workspace,
    focus: Option<egui::Id>,
    stopping: bool,
    stage: DictationStage,
}

enum DictationStage {
    Recording { audio: PathBuf },
    Transcribing { audio: PathBuf, output: PathBuf },
}

pub struct DictationResult {
    pub text: String,
    pub workspace: Workspace,
    pub focus: Option<egui::Id>,
}

pub struct Narration {
    preparing: Option<Receiver<Result<PathBuf, String>>>,
    cancelled: Arc<AtomicBool>,
    child: Option<Child>,
    text_path: Option<PathBuf>,
    voice: String,
    pub status: String,
}

impl Default for Narration {
    fn default() -> Self {
        Self {
            preparing: None,
            cancelled: Arc::new(AtomicBool::new(false)),
            child: None,
            text_path: None,
            voice: String::new(),
            status: String::new(),
        }
    }
}

impl Narration {
    pub fn busy(&self) -> bool {
        self.preparing.is_some() || self.child.is_some()
    }

    pub fn start(&mut self, source: ExportSource, page: usize, voice: &str) {
        self.stop();
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.voice = voice.to_string();
        self.status = format!("Preparing text from page {}…", page + 1);
        let (tx, rx) = mpsc::channel();
        let cancelled = self.cancelled.clone();
        std::thread::spawn(move || {
            let result = extract_text(source, page, &cancelled);
            if cancelled.load(Ordering::Relaxed) {
                if let Ok(path) = result {
                    let _ = fs::remove_file(path);
                }
            } else {
                let _ = tx.send(result);
            }
        });
        self.preparing = Some(rx);
    }

    pub fn poll(&mut self) {
        if let Some(result) = self.preparing.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.preparing = None;
            match result {
                Ok(path) => {
                    let mut child = Command::new("/usr/bin/say");
                    child.arg("-v").arg(&self.voice).arg("-f").arg(&path);
                    match child.spawn() {
                        Ok(child) => {
                            self.child = Some(child);
                            self.text_path = Some(path);
                            self.status = format!("Reading with {}", self.voice);
                        }
                        Err(error) => {
                            let _ = fs::remove_file(path);
                            self.status = format!("Could not start narration: {error}");
                        }
                    }
                }
                Err(error) => self.status = error,
            }
        }
        if let Some(result) = self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
        {
            self.child = None;
            self.clean_text();
            self.status = if result.success() {
                "Finished reading"
            } else {
                "Narration stopped"
            }
            .into();
        }
    }

    pub fn stop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.preparing = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.clean_text();
        self.status = "Narration stopped".into();
    }

    fn clean_text(&mut self) {
        if let Some(path) = self.text_path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

impl Drop for Narration {
    fn drop(&mut self) {
        self.stop();
    }
}

fn extract_text(
    source: ExportSource,
    page: usize,
    cancelled: &AtomicBool,
) -> Result<PathBuf, String> {
    if page >= source.pages {
        return Err("This PDF page is unavailable.".into());
    }
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let path = std::env::temp_dir().join(format!(
        "polymath-narration-{}-{}.txt",
        std::process::id(),
        u64::from_le_bytes(random)
    ));
    let result = (|| -> Result<(), String> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&path).map_err(|e| e.to_string())?;
        let mut out = BufWriter::new(file);
        let mut exporter = Exporter::from_source(source.clone());
        let mut found = false;
        for index in page..source.pages {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Narration cancelled".into());
            }
            let text = match exporter.text(index) {
                Ok(text) => text,
                Err(error) if error.contains("no text layer") => continue,
                Err(error) => return Err(error),
            };
            if !text.trim().is_empty() {
                writeln!(out, "{}\n", text.trim()).map_err(|e| e.to_string())?;
                found = true;
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        if !found {
            return Err(
                "This PDF has no readable text layer. Run OCR in Reader before narrating it."
                    .into(),
            );
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(path),
        Err(error) => {
            let _ = fs::remove_file(&path);
            Err(error)
        }
    }
}

pub struct Voice {
    dictation: Option<Dictation>,
    pub narration: Narration,
    pub language: String,
    pub voice: String,
    pub voices: Vec<(String, String)>,
    pub panel_open: bool,
    pub last_transcript: String,
    pub notice: String,
}

impl Default for Voice {
    fn default() -> Self {
        let voices = installed_voices();
        let voice = voices
            .iter()
            .find(|(name, _)| name == "Samantha")
            .or_else(|| voices.iter().find(|(_, language)| language == "en_US"))
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        Self {
            dictation: None,
            narration: Narration::default(),
            language: "auto".into(),
            voice,
            voices,
            panel_open: false,
            last_transcript: String::new(),
            notice: String::new(),
        }
    }
}

impl Voice {
    pub fn recording(&self) -> bool {
        self.dictation
            .as_ref()
            .is_some_and(|d| matches!(d.stage, DictationStage::Recording { .. }) && !d.stopping)
    }
    pub fn transcribing(&self) -> bool {
        self.dictation.as_ref().is_some_and(|d| d.stopping)
    }
    pub fn busy(&self) -> bool {
        self.dictation.is_some() || self.narration.busy()
    }
    pub fn start_dictation(
        &mut self,
        workspace: Workspace,
        focus: Option<egui::Id>,
    ) -> Result<(), String> {
        if self.dictation.is_some() {
            return Err("Dictation is already running.".into());
        }
        let helper = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("PolymathRecorder");
        if !helper.is_file() {
            return Err(
                "The local microphone helper is missing from this Polymath installation.".into(),
            );
        }
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let audio = std::env::temp_dir().join(format!(
            "polymath-dictation-{}-{}.wav",
            std::process::id(),
            u64::from_le_bytes(random)
        ));
        let child = Command::new(helper)
            .arg(&audio)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start dictation: {e}"))?;
        self.dictation = Some(Dictation {
            child,
            workspace,
            focus,
            stopping: false,
            stage: DictationStage::Recording { audio },
        });
        self.notice = "Listening locally. Press the microphone again to insert.".into();
        Ok(())
    }
    pub fn stop_dictation(&mut self) -> Result<(), String> {
        let Some(dictation) = self.dictation.as_mut() else {
            return Ok(());
        };
        if !dictation.stopping {
            dictation
                .child
                .stdin
                .take()
                .ok_or("Speech helper input closed")?
                .write_all(b"\n")
                .map_err(|e| e.to_string())?;
            dictation.stopping = true;
            self.notice = "Transcribing on this Mac…".into();
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Option<DictationResult> {
        self.narration.poll();
        let finished = self
            .dictation
            .as_mut()
            .and_then(|d| d.child.try_wait().ok())
            .is_some_and(|s| s.is_some());
        if !finished {
            return None;
        }
        let dictation = self.dictation.take()?;
        let target = (dictation.workspace, dictation.focus);
        match (dictation.stage, dictation.child.wait_with_output()) {
            (DictationStage::Recording { audio }, Ok(output)) if output.status.success() => {
                if !audio.starts_with(std::env::temp_dir())
                    || !audio.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        n.starts_with("polymath-dictation-") && n.ends_with(".wav")
                    })
                    || !audio.is_file()
                {
                    self.notice = "The microphone recording could not be found.".into();
                    self.panel_open = true;
                    return None;
                }
                match self.start_transcription(audio, target.0, target.1) {
                    Ok(()) => None,
                    Err(error) => {
                        self.notice = error;
                        self.panel_open = true;
                        None
                    }
                }
            }
            (DictationStage::Transcribing { audio, output }, Ok(result)) => {
                let transcript_file = transcript_path(&output);
                let text = fs::read_to_string(&transcript_file)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let _ = fs::remove_file(audio);
                let _ = fs::remove_file(transcript_file);
                if !result.status.success() {
                    self.notice = format!(
                        "Local transcription failed: {}",
                        String::from_utf8_lossy(&result.stderr).trim()
                    );
                    self.panel_open = true;
                    None
                } else if text.is_empty() {
                    self.notice = "No speech was recognized.".into();
                    self.panel_open = true;
                    None
                } else {
                    self.last_transcript = text.clone();
                    self.notice = "Dictation ready".into();
                    Some(DictationResult {
                        text,
                        workspace: target.0,
                        focus: target.1,
                    })
                }
            }
            (DictationStage::Recording { audio }, Ok(output)) => {
                let _ = fs::remove_file(audio);
                self.notice = String::from_utf8_lossy(&output.stderr).trim().to_string();
                self.panel_open = true;
                None
            }
            (DictationStage::Transcribing { audio, output }, Err(error)) => {
                let _ = fs::remove_file(audio);
                let _ = fs::remove_file(transcript_path(&output));
                self.notice = format!("Local transcription failed: {error}");
                self.panel_open = true;
                None
            }
            (DictationStage::Recording { audio }, Err(error)) => {
                let _ = fs::remove_file(audio);
                self.notice = format!("Microphone recording failed: {error}");
                self.panel_open = true;
                None
            }
        }
    }
    fn start_transcription(
        &mut self,
        audio: PathBuf,
        workspace: Workspace,
        focus: Option<egui::Id>,
    ) -> Result<(), String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let whisper = executable.with_file_name("PolymathWhisper");
        let model = executable
            .parent()
            .and_then(|p| p.parent())
            .ok_or("Could not locate Polymath's bundled speech model")?
            .join("Resources/models/ggml-small-q5_1.bin");
        if !whisper.is_file() || !model.is_file() {
            let _ = fs::remove_file(audio);
            return Err(
                "The local Whisper model is missing from this Polymath installation.".into(),
            );
        }
        let output = audio.with_extension("transcript");
        let language = self.language.split('-').next().unwrap_or("en");
        let child = Command::new(whisper)
            .args([
                "-m",
                model.to_str().ok_or("Invalid speech model path")?,
                "-f",
                audio.to_str().ok_or("Invalid audio path")?,
                "-l",
                language,
                "-nt",
                "-np",
                "-otxt",
                "-of",
                output.to_str().ok_or("Invalid transcript path")?,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                let _ = fs::remove_file(&audio);
                format!("Could not start local Whisper: {e}")
            })?;
        self.dictation = Some(Dictation {
            child,
            workspace,
            focus,
            stopping: true,
            stage: DictationStage::Transcribing { audio, output },
        });
        self.notice = "Transcribing with the bundled local Whisper model…".into();
        Ok(())
    }
    pub fn languages() -> &'static [(&'static str, &'static str)] {
        LANGUAGES
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        if let Some(mut dictation) = self.dictation.take() {
            let _ = dictation.child.kill();
            let _ = dictation.child.wait();
            match dictation.stage {
                DictationStage::Recording { audio } => {
                    let _ = fs::remove_file(audio);
                }
                DictationStage::Transcribing { audio, output } => {
                    let _ = fs::remove_file(audio);
                    let _ = fs::remove_file(transcript_path(&output));
                }
            }
        }
    }
}

fn transcript_path(output: &std::path::Path) -> PathBuf {
    PathBuf::from(format!("{}.txt", output.display()))
}

fn installed_voices() -> Vec<(String, String)> {
    let Ok(output) = Command::new("/usr/bin/say").args(["-v", "?"]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let before_sample = line.split_once('#')?.0.trim();
            let (name, language) = before_sample.rsplit_once(char::is_whitespace)?;
            let language = language.trim();
            if !language.contains('_') {
                return None;
            }
            Some((name.trim().to_string(), language.to_string()))
        })
        .collect()
}

pub fn request_repaint(ctx: &egui::Context, voice: &Voice) {
    if voice.busy() {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn blank_pdf_text_is_not_narrated() {
        let mut engine = pdfcraft_engine::Session::new();
        let bytes = engine.create_from_text("empty", " ").expect("pdf");
        let id = engine.open("empty", None, bytes, None).expect("open");
        let source = engine
            .docs()
            .iter()
            .find(|doc| doc.id == id)
            .expect("doc")
            .export_source();
        let cancelled = AtomicBool::new(false);
        let result = extract_text(source, 0, &cancelled);
        assert!(result.is_err());
    }
    #[test]
    fn extracts_current_pdf_text_for_offline_narration() {
        let mut engine = pdfcraft_engine::Session::new();
        let bytes = engine
            .create_from_text("book", "The lantern was left beside the door.")
            .expect("pdf");
        let id = engine.open("book", None, bytes, None).expect("open");
        let source = engine
            .docs()
            .iter()
            .find(|doc| doc.id == id)
            .expect("doc")
            .export_source();
        let path = extract_text(source, 0, &AtomicBool::new(false)).expect("extract");
        let text = fs::read_to_string(&path).expect("read");
        let _ = fs::remove_file(path);
        assert!(text.contains("lantern was left"), "{text}");
    }
}
