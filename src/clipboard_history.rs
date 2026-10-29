//! Bounded local Clipboard History companion and global shortcut.
use egui::Context;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub(crate) struct ClipboardHistory {
    pub enabled: bool,
    pub shortcut: String,
    pub status: String,
    pub recording_shortcut: bool,
    child: Option<Child>,
    lines: Option<Receiver<String>>,
}

impl Default for ClipboardHistory {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut: "cmd+d".into(),
            status: "Starting…".into(),
            recording_shortcut: false,
            child: None,
            lines: None,
        }
    }
}

impl ClipboardHistory {
    pub fn display_shortcut(&self) -> String {
        let mut display = String::new();
        let parts: Vec<_> = self.shortcut.split('+').collect();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            display.push_str(match *part {
                "ctrl" => "⌃",
                "option" => "⌥",
                "shift" => "⇧",
                "cmd" => "⌘",
                _ => "",
            });
        }
        if let Some(key) = parts.last() {
            display.push_str(&key.to_ascii_uppercase());
        }
        display
    }

    pub fn set_enabled(&mut self, enabled: bool, ctx: &Context) {
        self.enabled = enabled;
        if enabled {
            self.start(ctx);
        } else {
            self.stop();
            self.status = "Paused".into();
        }
    }

    pub fn start(&mut self, ctx: &Context) {
        if !self.enabled || self.child.is_some() {
            return;
        }
        let helper = std::env::var_os("POLYMATH_CLIPBOARD_HELPER")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .map(|exe| exe.with_file_name("PolymathGlobalDictation"))
            });
        let Some(helper) = helper.filter(|path| path.is_file()) else {
            self.status = "Clipboard helper is missing from this installation.".into();
            return;
        };
        let child = Command::new(helper)
            .arg("--clipboard")
            .arg(std::process::id().to_string())
            .arg(&self.shortcut)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            self.status = "Could not start Clipboard History.".into();
            return;
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            self.status = "Clipboard helper did not open its status channel.".into();
            return;
        };
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        if std::thread::Builder::new()
            .name("Polymath clipboard status".into())
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
            self.status = "Could not watch Clipboard History status.".into();
            return;
        }
        self.child = Some(child);
        self.lines = Some(rx);
        self.status = "Starting…".into();
    }

    pub fn set_shortcut(&mut self, shortcut: String) {
        self.shortcut = shortcut;
        self.send(json!({"shortcut":self.shortcut}));
    }

    pub fn show(&mut self, ctx: &Context) {
        if !self.enabled {
            self.set_enabled(true, ctx);
        }
        self.send(json!({"show":true}));
    }

    pub fn poll(&mut self) {
        if let Some(lines) = &self.lines {
            while let Ok(line) = lines.try_recv() {
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && value.get("kind").and_then(Value::as_str) == Some("status")
                    && let Some(status) = value.get("text").and_then(Value::as_str)
                {
                    self.status = status.to_owned();
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
                self.status = "Clipboard helper stopped. Turn it off and on to retry.".into();
            }
        }
    }

    fn send(&mut self, value: Value) {
        if let Some(input) = self.child.as_mut().and_then(|child| child.stdin.as_mut()) {
            let _ = input.write_all((value.to_string() + "\n").as_bytes());
            let _ = input.flush();
        }
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

impl Drop for ClipboardHistory {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_shortcuts_for_settings() {
        let mut history = ClipboardHistory::default();
        assert_eq!(history.display_shortcut(), "⌘D");
        history.shortcut = "ctrl+option+shift+v".into();
        assert_eq!(history.display_shortcut(), "⌃⌥⇧V");
    }
}
