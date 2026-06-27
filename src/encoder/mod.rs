use anyhow::{Context, Result};
use ffmpeg_sidecar::{child::FfmpegChild, command::FfmpegCommand};
use image::RgbaImage;
use std::io::Write;
use std::process::ChildStdin;

pub struct VideoEncoder {
    process: FfmpegChild,
    stdin: ChildStdin,
    width: u32,
    height: u32,
}

impl VideoEncoder {
    pub fn new(output_path: &str, width: u32, height: u32, fps: u32) -> Result<Self> {
        let mut process = FfmpegCommand::new()
            .hide_banner()
            .args(["-loglevel", "error"])
            .args([
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
                &format!("{}x{}", width, height),
                "-framerate",
                &fps.to_string(),
            ])
            .input("pipe:0")
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-preset", "fast"])
            .overwrite()
            .arg(output_path)
            .spawn()
            .context("Failed to spawn FFmpeg. Is FFmpeg installed on your system?")?;

        let stdin = process
            .take_stdin()
            .context("Failed to acquire FFmpeg stdin")?;

        Ok(Self {
            process,
            stdin,
            width,
            height,
        })
    }

    pub fn write_frame(&mut self, frame: &RgbaImage) -> Result<()> {
        if frame.width() != self.width || frame.height() != self.height {
            anyhow::bail!(
                "Frame size {}x{} does not match encoder size {}x{}",
                frame.width(),
                frame.height(),
                self.width,
                self.height
            );
        }

        self.stdin
            .write_all(frame.as_raw())
            .context("Failed to write frame to FFmpeg")?;

        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        // Closing stdin signals EOF to FFmpeg.
        drop(self.stdin);

        let status = self.process.wait()?;

        if !status.success() {
            anyhow::bail!("FFmpeg exited with status {}", status);
        }

        Ok(())
    }
}
