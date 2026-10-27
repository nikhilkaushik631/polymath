//! Explicit, local video exports through an installed FFmpeg CLI.
//! Paths are passed as process arguments; no shell, server, or network is used.

use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

static WORK_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct Clip {
    pub path: PathBuf,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug)]
pub struct AudioTrack {
    pub path: PathBuf,
    /// Placement in the output timeline, in seconds.
    pub start: f64,
    pub gain: f32,
    pub replace_original: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Original,
    High,
    #[default]
    Balanced,
    Small,
}

impl Quality {
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Original size",
            Self::High => "High quality",
            Self::Balanced => "Balanced",
            Self::Small => "Small file",
        }
    }

    fn crf(self) -> u8 {
        match self {
            Self::Original => 18,
            Self::High => 21,
            Self::Balanced => 24,
            Self::Small => 29,
        }
    }

    fn max_edge(self) -> u32 {
        match self {
            Self::Original => 3840,
            Self::High => 1920,
            Self::Balanced => 1280,
            Self::Small => 854,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub clips: Vec<Clip>,
    pub audio: Option<AudioTrack>,
    pub mute_original: bool,
    pub output: PathBuf,
    pub quality: Quality,
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f64>,
    pub has_video: bool,
    pub has_audio: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportPhase {
    Validating,
    Encoding {
        clip_index: usize,
        clip_count: usize,
    },
    Joining,
    AddingAudio,
    Publishing,
    Done,
    Failed,
    Cancelled,
}

impl ExportPhase {
    pub fn label(&self) -> String {
        match self {
            Self::Validating => "Checking clips".into(),
            Self::Encoding {
                clip_index,
                clip_count,
            } => {
                format!("Processing clip {} of {}", clip_index + 1, clip_count)
            }
            Self::Joining => "Joining clips".into(),
            Self::AddingAudio => "Adding audio".into(),
            Self::Publishing => "Saving video".into(),
            Self::Done => "Export complete".into(),
            Self::Failed => "Export failed".into(),
            Self::Cancelled => "Export cancelled".into(),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug)]
pub struct ExportStatus {
    pub phase: ExportPhase,
    pub fraction: f32,
    pub output: Option<PathBuf>,
    pub error: Option<String>,
}

impl ExportStatus {
    fn new(phase: ExportPhase, fraction: f32) -> Self {
        Self {
            phase,
            fraction: fraction.clamp(0.0, 1.0),
            output: None,
            error: None,
        }
    }
}

pub struct ExportJob {
    updates: mpsc::Receiver<ExportStatus>,
    cancel_flag: Arc<AtomicBool>,
    status: ExportStatus,
}

impl ExportJob {
    /// Starts processing only after the user explicitly requests an export.
    pub fn spawn(request: ExportRequest) -> Result<Self, String> {
        if request.clips.is_empty() {
            return Err("Add at least one clip before exporting".into());
        }
        let (tx, updates) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel_flag);
        thread::Builder::new()
            .name("polymath-video-export".into())
            .spawn(move || {
                let result = export(&request, &tx, &worker_cancel);
                let status = match result {
                    Ok(()) => ExportStatus {
                        phase: ExportPhase::Done,
                        fraction: 1.0,
                        output: Some(request.output),
                        error: None,
                    },
                    Err(PipelineError::Cancelled) => ExportStatus::new(ExportPhase::Cancelled, 0.0),
                    Err(PipelineError::Message(error)) => ExportStatus {
                        phase: ExportPhase::Failed,
                        fraction: 0.0,
                        output: None,
                        error: Some(error),
                    },
                };
                let _ = tx.send(status);
            })
            .map_err(|error| format!("Could not start video export: {error}"))?;
        Ok(Self {
            updates,
            cancel_flag,
            status: ExportStatus::new(ExportPhase::Validating, 0.0),
        })
    }

    /// Nonblocking; drain updates once per UI frame.
    pub fn poll(&mut self) -> ExportStatus {
        while let Ok(update) = self.updates.try_recv() {
            self.status = update;
        }
        self.status.clone()
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

enum PipelineError {
    Cancelled,
    Message(String),
}

impl From<String> for PipelineError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

fn fail(error: impl std::fmt::Display) -> PipelineError {
    PipelineError::Message(error.to_string())
}

fn tool_path(name: &str) -> Result<PathBuf, String> {
    for prefix in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let path = Path::new(prefix).join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let path = dir.join(name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err(format!(
        "{name} is required for video editing. Install FFmpeg locally, then try again."
    ))
}

/// Inspects a local media file on demand. No persistent process is started.
pub fn probe(path: &Path) -> Result<Probe, String> {
    let path = local_file(path)?;
    let ffprobe = tool_path("ffprobe")?;
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe,fd",
            "-show_entries",
            "format=duration:stream=codec_type,codec_name,width,height,avg_frame_rate,duration",
            "-of",
            "json",
            "-i",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("Cannot inspect media: {error}"))?;
    if !output.status.success() {
        return Err(format!("Cannot inspect media: {}", brief(&output.stderr)));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Cannot read media information: {error}"))?;
    let streams = value["streams"]
        .as_array()
        .ok_or("Media contains no readable streams")?;
    let video = streams.iter().find(|s| s["codec_type"] == "video");
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let duration = number(&value["format"]["duration"])
        .or_else(|| {
            streams
                .iter()
                .filter_map(|s| number(&s["duration"]))
                .reduce(f64::max)
        })
        .ok_or("Media duration is unavailable")?;
    if !duration.is_finite() || duration <= 0.0 {
        return Err("Media duration is invalid".into());
    }
    let fps = video
        .and_then(|s| s["avg_frame_rate"].as_str())
        .and_then(parse_rate);
    Ok(Probe {
        duration,
        width: video
            .and_then(|s| s["width"].as_u64())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        height: video
            .and_then(|s| s["height"].as_u64())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        fps,
        has_video: video.is_some(),
        has_audio: audio.is_some(),
        video_codec: video
            .and_then(|s| s["codec_name"].as_str())
            .map(str::to_owned),
        audio_codec: audio
            .and_then(|s| s["codec_name"].as_str())
            .map(str::to_owned),
    })
}

/// Returns an encoded PNG thumbnail for the editor timeline.
pub fn thumbnail(path: &Path, seconds: f64, max_width: u32) -> Result<Vec<u8>, String> {
    let path = local_file(path)?;
    let info = probe(&path)?;
    if !info.has_video {
        return Err("This file has no video frames".into());
    }
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("Invalid thumbnail time".into());
    }
    let ffmpeg = tool_path("ffmpeg")?;
    let width = max_width.clamp(80, 1024);
    let time = seconds.min((info.duration - 0.05).max(0.0));
    let output = Command::new(ffmpeg)
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe,fd",
            "-ss",
        ])
        .arg(format!("{time:.3}"))
        .arg("-i")
        .arg(path)
        .args(["-frames:v", "1", "-vf"])
        .arg(format!("scale={width}:-2"))
        .args(["-f", "image2pipe", "-vcodec", "png", "pipe:1"])
        .output()
        .map_err(|error| format!("Cannot make thumbnail: {error}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(format!("Cannot make thumbnail: {}", brief(&output.stderr)));
    }
    Ok(output.stdout)
}

fn local_file(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Choose a local file".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Cannot access file: {error}"))?;
    if !canonical.is_file() {
        return Err("Choose a regular local file".into());
    }
    Ok(canonical)
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str()?.parse().ok())
}

fn parse_rate(rate: &str) -> Option<f64> {
    let (numerator, denominator) = rate.split_once('/')?;
    let numerator: f64 = numerator.parse().ok()?;
    let denominator: f64 = denominator.parse().ok()?;
    let fps = numerator / denominator;
    (fps.is_finite() && fps > 0.0).then_some(fps)
}

fn brief(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    let start = text.len().saturating_sub(600);
    text.get(start..).unwrap_or(text).to_owned()
}

struct ValidatedClip {
    path: PathBuf,
    start: f64,
    duration: f64,
    has_audio: bool,
}

struct ValidatedExport {
    clips: Vec<ValidatedClip>,
    audio: Option<AudioTrack>,
    output: PathBuf,
    total_duration: f64,
    width: u32,
    height: u32,
    fps: u32,
}

fn validate(request: &ExportRequest) -> Result<ValidatedExport, PipelineError> {
    if request.clips.is_empty() || request.clips.len() > 100 {
        return Err(fail("Choose between 1 and 100 video clips"));
    }
    if !request.output.is_absolute()
        || !request
            .output
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
    {
        return Err(fail("Choose an absolute .mp4 output path"));
    }
    let parent = request
        .output
        .parent()
        .ok_or_else(|| fail("Choose an output folder"))?;
    let parent = parent
        .canonicalize()
        .map_err(|error| fail(format!("Cannot access output folder: {error}")))?;
    if !parent.is_dir() {
        return Err(fail("Output folder is not a directory"));
    }
    let filename = request
        .output
        .file_name()
        .ok_or_else(|| fail("Choose an output file name"))?;
    let output = parent.join(filename);
    if fs::symlink_metadata(&output).is_ok() {
        return Err(fail("Output file already exists. Choose a new name."));
    }

    let mut clips = Vec::with_capacity(request.clips.len());
    let mut first = None;
    let mut total_duration = 0.0;
    for (index, clip) in request.clips.iter().enumerate() {
        let path = local_file(&clip.path).map_err(fail)?;
        if path == output {
            return Err(fail("Output cannot replace a source clip"));
        }
        if !clip.start.is_finite()
            || !clip.end.is_finite()
            || clip.start < 0.0
            || clip.end - clip.start < 0.1
        {
            return Err(fail(format!("Clip {} has invalid trim points", index + 1)));
        }
        let info = probe(&path).map_err(fail)?;
        if !info.has_video || info.width == 0 || info.height == 0 {
            return Err(fail(format!("Clip {} has no readable video", index + 1)));
        }
        if clip.end > info.duration + 0.05 {
            return Err(fail(format!(
                "Clip {} ends after the source video",
                index + 1
            )));
        }
        first.get_or_insert_with(|| info.clone());
        let duration = clip.end - clip.start;
        total_duration += duration;
        clips.push(ValidatedClip {
            path,
            start: clip.start,
            duration,
            has_audio: use_source_audio(info.has_audio, request.mute_original),
        });
    }
    if total_duration > 24.0 * 60.0 * 60.0 {
        return Err(fail("Export is limited to 24 hours"));
    }

    let audio = if let Some(track) = &request.audio {
        let path = local_file(&track.path).map_err(fail)?;
        let info = probe(&path).map_err(fail)?;
        if !info.has_audio {
            return Err(fail("Selected soundtrack has no audio"));
        }
        if !track.start.is_finite() || track.start < 0.0 || track.start >= total_duration {
            return Err(fail("Soundtrack start is outside the video timeline"));
        }
        if !track.gain.is_finite() || !(0.0..=2.0).contains(&track.gain) {
            return Err(fail("Soundtrack volume must be between 0% and 200%"));
        }
        Some(AudioTrack {
            path,
            ..track.clone()
        })
    } else {
        None
    };

    let first = first.ok_or_else(|| fail("No video clips selected"))?;
    let (width, height) = output_size(first.width, first.height, request.quality.max_edge());
    let fps = if request.quality == Quality::Original || request.quality == Quality::High {
        first.fps.unwrap_or(30.0).round().clamp(24.0, 60.0) as u32
    } else {
        30
    };
    Ok(ValidatedExport {
        clips,
        audio,
        output,
        total_duration,
        width,
        height,
        fps,
    })
}

fn use_source_audio(source_has_audio: bool, mute_original: bool) -> bool {
    source_has_audio && !mute_original
}

fn output_size(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let largest = width.max(height).max(1);
    let scale = f64::from(max_edge) / f64::from(largest);
    let scale = scale.min(1.0);
    let even = |value: u32| (value.max(2) / 2) * 2;
    (
        even((f64::from(width) * scale).round() as u32),
        even((f64::from(height) * scale).round() as u32),
    )
}

struct WorkDir(PathBuf);

impl WorkDir {
    fn create(output: &Path) -> Result<Self, PipelineError> {
        let parent = output
            .parent()
            .ok_or_else(|| fail("Output folder is unavailable"))?;
        for _ in 0..16 {
            let id = WORK_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".polymath-video-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(fail(format!("Cannot prepare export folder: {error}"))),
            }
        }
        Err(fail("Could not create an export workspace"))
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn export(
    request: &ExportRequest,
    updates: &mpsc::Sender<ExportStatus>,
    cancel: &AtomicBool,
) -> Result<(), PipelineError> {
    check_cancel(cancel)?;
    let _ = updates.send(ExportStatus::new(ExportPhase::Validating, 0.0));
    let validated = validate(request)?;
    let ffmpeg = tool_path("ffmpeg").map_err(fail)?;
    let work = WorkDir::create(&validated.output)?;
    let count = validated.clips.len();

    for (index, clip) in validated.clips.iter().enumerate() {
        check_cancel(cancel)?;
        let segment = work.0.join(format!("segment-{index:04}.mp4"));
        let phase = ExportPhase::Encoding {
            clip_index: index,
            clip_count: count,
        };
        let base = 0.02 + 0.80 * index as f32 / count as f32;
        let span = 0.80 / count as f32;
        let mut cmd = Command::new(&ffmpeg);
        cmd.args([
            "-nostdin",
            "-v",
            "error",
            "-stats_period",
            "0.2",
            "-protocol_whitelist",
            "file,pipe,fd",
            "-ss",
        ])
        .arg(format!("{:.6}", clip.start))
        .arg("-i")
        .arg(&clip.path);
        if !clip.has_audio {
            cmd.args([
                "-f",
                "lavfi",
                "-i",
                "anullsrc=channel_layout=stereo:sample_rate=48000",
            ]);
        }
        cmd.args(["-map", "0:v:0"]);
        if clip.has_audio {
            cmd.args(["-map", "0:a:0"]);
        } else {
            cmd.args(["-map", "1:a:0"]);
        }
        cmd.arg("-vf").arg(format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={},format=yuv420p",
            validated.width, validated.height, validated.width, validated.height, validated.fps
        ));
        cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-crf"])
            .arg(request.quality.crf().to_string())
            .args([
                "-c:a", "aac", "-b:a", "160k", "-ar", "48000", "-ac", "2", "-af", "apad", "-t",
            ])
            .arg(format!("{:.6}", clip.duration))
            .args(["-movflags", "+faststart", "-progress", "pipe:1", "-n"])
            .arg(segment);
        run_ffmpeg(
            &mut cmd,
            &work.0,
            updates,
            cancel,
            phase,
            base,
            span,
            clip.duration,
        )?;
    }

    check_cancel(cancel)?;
    let _ = updates.send(ExportStatus::new(ExportPhase::Joining, 0.83));
    let joined = if count == 1 {
        work.0.join("segment-0000.mp4")
    } else {
        let manifest = work.0.join("concat.ffconcat");
        let mut file = File::create(&manifest)
            .map_err(|error| fail(format!("Cannot create clip list: {error}")))?;
        writeln!(file, "ffconcat version 1.0").map_err(|error| fail(error))?;
        for index in 0..count {
            writeln!(file, "file 'segment-{index:04}.mp4'").map_err(|error| fail(error))?;
        }
        let joined = work.0.join("joined.mp4");
        let mut cmd = Command::new(&ffmpeg);
        cmd.current_dir(&work.0)
            .args([
                "-nostdin",
                "-v",
                "error",
                "-protocol_whitelist",
                "file,pipe,fd",
                "-f",
                "concat",
                "-safe",
                "1",
                "-i",
            ])
            .arg(&manifest)
            .args([
                "-c",
                "copy",
                "-movflags",
                "+faststart",
                "-progress",
                "pipe:1",
                "-n",
            ])
            .arg(&joined);
        run_ffmpeg(
            &mut cmd,
            &work.0,
            updates,
            cancel,
            ExportPhase::Joining,
            0.83,
            0.07,
            validated.total_duration,
        )?;
        joined
    };

    let finished = if let Some(audio) = &validated.audio {
        check_cancel(cancel)?;
        let mixed = work.0.join("mixed.mp4");
        let filter = soundtrack_filter(audio);
        let mut cmd = Command::new(&ffmpeg);
        cmd.args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe,fd",
            "-i",
        ])
        .arg(&joined)
        .args(["-stream_loop", "-1", "-protocol_whitelist", "file,pipe,fd"])
        .arg("-i")
        .arg(&audio.path)
        .arg("-filter_complex")
        .arg(filter)
        .args([
            "-map", "0:v:0", "-map", "[a]", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-t",
        ])
        .arg(format!("{:.6}", validated.total_duration))
        .args(["-movflags", "+faststart", "-progress", "pipe:1", "-n"])
        .arg(&mixed);
        run_ffmpeg(
            &mut cmd,
            &work.0,
            updates,
            cancel,
            ExportPhase::AddingAudio,
            0.90,
            0.08,
            validated.total_duration,
        )?;
        mixed
    } else {
        joined
    };

    check_cancel(cancel)?;
    let _ = updates.send(ExportStatus::new(ExportPhase::Publishing, 0.99));
    // The workdir is in the destination folder, so hard-linking publishes atomically
    // and fails if another process created the requested name meanwhile.
    fs::hard_link(&finished, &validated.output).map_err(|error| {
        if fs::symlink_metadata(&validated.output).is_ok() {
            fail("Output file already exists. Choose a new name.")
        } else {
            fail(format!("Cannot save video: {error}"))
        }
    })?;
    Ok(())
}

fn soundtrack_filter(audio: &AudioTrack) -> String {
    let delay_ms = (audio.start * 1_000.0).round() as u64;
    let prefix = format!("[1:a]volume={:.3},adelay={delay_ms}:all=1", audio.gain);
    if audio.replace_original {
        format!("{prefix}[a]")
    } else {
        format!("{prefix}[music];[0:a][music]amix=inputs=2:duration=first:dropout_transition=0[a]")
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), PipelineError> {
    if cancel.load(Ordering::Relaxed) {
        Err(PipelineError::Cancelled)
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn run_ffmpeg(
    cmd: &mut Command,
    work: &Path,
    updates: &mpsc::Sender<ExportStatus>,
    cancel: &AtomicBool,
    phase: ExportPhase,
    base: f32,
    span: f32,
    duration: f64,
) -> Result<(), PipelineError> {
    let log_path = work.join("ffmpeg.log");
    let log = File::create(&log_path)
        .map_err(|error| fail(format!("Cannot write export log: {error}")))?;
    let _ = updates.send(ExportStatus::new(phase.clone(), base));
    let mut child: Child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|error| fail(format!("Could not start FFmpeg: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| fail("FFmpeg progress is unavailable"))?;
    let progress_tx = updates.clone();
    let progress_phase = phase.clone();
    let progress_reader = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(seconds) = parse_progress(&line) {
                let fraction = base + span * (seconds / duration).clamp(0.0, 1.0) as f32;
                let _ = progress_tx.send(ExportStatus::new(progress_phase.clone(), fraction));
            }
        }
    });
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(PipelineError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(_)) => {
                let mut bytes = Vec::new();
                if let Ok(mut file) = File::open(&log_path) {
                    let _ = file.read_to_end(&mut bytes);
                }
                let detail = brief(&bytes);
                break Err(fail(if detail.is_empty() {
                    "FFmpeg could not process this file".into()
                } else {
                    format!("FFmpeg could not process this file: {detail}")
                }));
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => break Err(fail(format!("Could not monitor FFmpeg: {error}"))),
        }
    };
    let _ = progress_reader.join();
    if result.is_ok() {
        let _ = updates.send(ExportStatus::new(phase, base + span));
    }
    result
}

fn parse_progress(line: &str) -> Option<f64> {
    if let Some(value) = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))
    {
        return value.parse::<f64>().ok().map(|v| v / 1_000_000.0);
    }
    let value = line.strip_prefix("out_time=")?;
    let mut pieces = value.split(':');
    let hours: f64 = pieces.next()?.parse().ok()?;
    let minutes: f64 = pieces.next()?.parse().ok()?;
    let seconds: f64 = pieces.next()?.parse().ok()?;
    (pieces.next().is_none()).then_some(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_are_even_and_do_not_upscale() {
        assert_eq!(output_size(1920, 1080, 1280), (1280, 720));
        assert_eq!(output_size(319, 201, 3840), (318, 200));
        assert_eq!(output_size(1080, 1920, 854), (480, 854));
    }

    #[test]
    fn progress_lines_use_seconds() {
        assert_eq!(parse_progress("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_progress("out_time=00:01:02.500000"), Some(62.5));
        assert_eq!(parse_progress("progress=continue"), None);
    }

    #[test]
    fn terminal_statuses_are_clear() {
        assert!(ExportPhase::Done.is_terminal());
        assert!(ExportPhase::Failed.is_terminal());
        assert!(ExportPhase::Cancelled.is_terminal());
        assert!(!ExportPhase::Joining.is_terminal());
    }

    #[test]
    fn muting_uses_silence_even_when_source_has_audio() {
        assert!(use_source_audio(true, false));
        assert!(!use_source_audio(true, true));
        assert!(!use_source_audio(false, false));
    }

    #[test]
    fn soundtrack_start_delays_audio_on_output_timeline() {
        let audio = AudioTrack {
            path: PathBuf::from("/tmp/music.mp3"),
            start: 1.5,
            gain: 0.75,
            replace_original: true,
        };
        assert_eq!(
            soundtrack_filter(&audio),
            "[1:a]volume=0.750,adelay=1500:all=1[a]"
        );
    }
}
