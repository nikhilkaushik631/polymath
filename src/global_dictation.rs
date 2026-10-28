//! Local macOS hold-to-dictate companion. No service starts until enabled.
use egui::Context;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub(crate) struct GlobalTranscript {
    pub text: String,
    pub pasted: bool,
}

pub(crate) struct GlobalDictation {
    pub enabled: bool,
    pub status: String,
    child: Option<Child>,
    lines: Option<Receiver<String>>,
    sent_language: String,
}

impl Default for GlobalDictation {
    fn default() -> Self {
        Self {
            enabled: false,
            status: "Off".into(),
            child: None,
            lines: None,
            sent_language: String::new(),
        }
    }
}

impl GlobalDictation {
    pub fn set_enabled(&mut self, enabled: bool, ctx: &Context, language: &str) {
        self.enabled = enabled;
        if !enabled {
            self.stop();
            self.status = "Off".into();
            return;
        }
        if self.child.is_some() {
            self.sync_language(language);
            return;
        }
        let helper = std::env::var_os("POLYMATH_GLOBAL_DICTATION_HELPER")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .map(|exe| exe.with_file_name("PolymathGlobalDictation"))
            });
        let Some(helper) = helper.filter(|path| path.is_file()) else {
            self.status = "Global dictation helper is missing from this installation.".into();
            return;
        };
        let child = Command::new(helper)
            .arg(language)
            .arg(std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            self.status = "Could not start global dictation helper.".into();
            return;
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            self.status = "Global dictation helper did not open its status channel.".into();
            return;
        };
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        if std::thread::Builder::new()
            .name("Polymath global dictation status".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                    wake.request_repaint();
                }
            })
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            self.status = "Could not watch global dictation status.".into();
            return;
        }
        self.child = Some(child);
        self.lines = Some(rx);
        self.sent_language = language.into();
        self.status = "Starting global dictation…".into();
    }

    pub fn sync_language(&mut self, language: &str) {
        if self.sent_language == language {
            return;
        }
        if let Some(input) = self.child.as_mut().and_then(|child| child.stdin.as_mut()) {
            let line = json!({"language":language}).to_string() + "\n";
            if input.write_all(line.as_bytes()).is_ok() {
                self.sent_language = language.into();
            }
        }
    }

    pub fn poll(&mut self) -> Option<GlobalTranscript> {
        let mut transcript = None;
        if let Some(lines) = &self.lines {
            while let Ok(line) = lines.try_recv() {
                if let Some(event) = parse_event(&line) {
                    match event {
                        Event::Status(status) => self.status = status,
                        Event::Transcript(result) => transcript = Some(result),
                    }
                }
            }
        }
        if self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some()
        {
            self.child = None;
            self.lines = None;
            if self.enabled {
                self.status = "Global shortcut stopped. Turn it off and on again to retry.".into();
            }
        }
        transcript
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Some(input) = child.stdin.as_mut() {
                let _ = input.write_all(b"{\"quit\":true}\n");
                let _ = input.flush();
            }
            for _ in 0..10 {
                if child.try_wait().ok().flatten().is_some() {
                    self.lines = None;
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        self.lines = None;
    }
}

impl Drop for GlobalDictation {
    fn drop(&mut self) {
        self.stop();
    }
}

enum Event {
    Status(String),
    Transcript(GlobalTranscript),
}

fn parse_event(line: &str) -> Option<Event> {
    let value: Value = serde_json::from_str(line).ok()?;
    let text = value.get("text")?.as_str()?.to_owned();
    match value.get("kind")?.as_str()? {
        "status" => Some(Event::Status(text)),
        "transcript" => Some(Event::Transcript(GlobalTranscript {
            text,
            pasted: value.get("pasted")?.as_bool()?,
        })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_transcript_messages_are_distinct() {
        assert!(matches!(
            parse_event(r#"{"kind":"status","text":"Ready"}"#),
            Some(Event::Status(status)) if status == "Ready"
        ));
        assert!(matches!(
            parse_event(r#"{"kind":"transcript","text":"नमस्ते","pasted":true}"#),
            Some(Event::Transcript(GlobalTranscript { text, pasted: true })) if text == "नमस्ते"
        ));
        assert!(parse_event("not json").is_none());
    }
}
