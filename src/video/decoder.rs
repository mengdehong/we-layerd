use std::{
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::{Child, ChildStdout, Command, Stdio},
};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bgra: Vec<u8>,
}

pub struct VideoPlayer {
    decoder: VideoDecoder,
    current: Option<VideoFrame>,
}

impl VideoPlayer {
    pub fn new(video_file: &Path) -> Result<Self> {
        let decoder = VideoDecoder::open(video_file.to_path_buf(), true)?;
        Ok(Self { decoder, current: None })
    }

    pub fn source_fps(&self) -> Option<f64> {
        self.decoder.estimated_fps()
    }

    pub fn advance_frame(&mut self) -> Result<bool> {
        match self.decoder.decode_next_frame()? {
            Some(frame) => {
                self.current = Some(frame);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn current_frame(&self) -> Option<&VideoFrame> {
        self.current.as_ref()
    }
}

struct VideoDecoder {
    path: PathBuf,
    loop_playback: bool,
    width: u32,
    height: u32,
    source_fps: Option<f64>,
    ffmpeg: Child,
    ffmpeg_stdout: ChildStdout,
}

impl VideoDecoder {
    fn open(path: PathBuf, loop_playback: bool) -> Result<Self> {
        let meta = probe_video_metadata(&path)?;
        let (ffmpeg, ffmpeg_stdout) = spawn_ffmpeg_decoder(&path, loop_playback)?;

        Ok(Self {
            path,
            loop_playback,
            width: meta.width,
            height: meta.height,
            source_fps: meta.fps,
            ffmpeg,
            ffmpeg_stdout,
        })
    }

    fn estimated_fps(&self) -> Option<f64> {
        self.source_fps
    }

    fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>> {
        let frame_size =
            self.width.checked_mul(self.height).and_then(|px| px.checked_mul(4)).ok_or_else(
                || anyhow!("video dimensions overflow: {}x{}", self.width, self.height),
            )? as usize;

        let mut frame = vec![0u8; frame_size];
        match self.ffmpeg_stdout.read_exact(&mut frame) {
            Ok(()) => Ok(Some(VideoFrame {
                width: self.width,
                height: self.height,
                stride: self.width * 4,
                bgra: frame,
            })),
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
                if self.loop_playback {
                    self.restart_ffmpeg()?;
                    return self.decode_next_frame();
                }
                Ok(None)
            }
            Err(err) => Err(anyhow!("failed reading decoded frame from ffmpeg: {err}")),
        }
    }

    fn restart_ffmpeg(&mut self) -> Result<()> {
        if let Err(err) = self.ffmpeg.kill() {
            let _ = err;
        }
        let _ = self.ffmpeg.wait();

        let (ffmpeg, ffmpeg_stdout) = spawn_ffmpeg_decoder(&self.path, self.loop_playback)?;
        self.ffmpeg = ffmpeg;
        self.ffmpeg_stdout = ffmpeg_stdout;
        Ok(())
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        let _ = self.ffmpeg.kill();
        let _ = self.ffmpeg.wait();
    }
}

struct VideoMetadata {
    width: u32,
    height: u32,
    fps: Option<f64>,
}

fn probe_video_metadata(path: &Path) -> Result<VideoMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &path.display().to_string(),
        ])
        .output()
        .with_context(|| format!("failed to launch ffprobe for {}", path.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();

    if lines.len() < 3 {
        return Err(anyhow!(
            "ffprobe metadata parse failed for {}: expected 3 lines (width/height/fps), got {}",
            path.display(),
            lines.len()
        ));
    }

    let width =
        lines[0].parse::<u32>().with_context(|| format!("invalid ffprobe width: {}", lines[0]))?;
    let height =
        lines[1].parse::<u32>().with_context(|| format!("invalid ffprobe height: {}", lines[1]))?;
    let fps = parse_ffmpeg_fraction(&lines[2]);

    Ok(VideoMetadata { width, height, fps })
}

fn parse_ffmpeg_fraction(raw: &str) -> Option<f64> {
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let num = parts[0].parse::<f64>().ok()?;
    let den = parts[1].parse::<f64>().ok()?;
    if num <= 0.0 || den <= 0.0 {
        return None;
    }
    Some(num / den)
}

fn spawn_ffmpeg_decoder(path: &Path, loop_playback: bool) -> Result<(Child, ChildStdout)> {
    let mut command = Command::new("ffmpeg");
    command.arg("-v").arg("error");
    if loop_playback {
        command.arg("-stream_loop").arg("-1");
    }
    command
        .arg("-i")
        .arg(path)
        .args([
            "-an",
            "-sn",
            "-dn",
            "-vf",
            "format=bgra",
            "-pix_fmt",
            "bgra",
            "-f",
            "rawvideo",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to launch ffmpeg for {}", path.display()))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("failed to capture ffmpeg stdout"))?;

    Ok((child, stdout))
}
