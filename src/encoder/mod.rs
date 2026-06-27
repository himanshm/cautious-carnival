use anyhow::{Context, Result};
use ffmpeg_sidecar::{child::FfmpegChild, command::FfmpegCommand};
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
            .output(output_path)
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

    pub fn write_frame(&mut self, frame_data: &[u8]) -> Result<()> {
        let expected_len = (self.width * self.height * 4) as usize;
        if frame_data.len() != expected_len {
            anyhow::bail!(
                "Frame data length {} does not match expected {} ({}x{}x4)",
                frame_data.len(),
                expected_len,
                self.width,
                self.height
            );
        }

        self.stdin
            .write_all(frame_data)
            .context("Failed to write frame to FFmpeg")?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        drop(self.stdin);
        let status = self.process.wait()?;
        if !status.success() {
            anyhow::bail!("FFmpeg exited with status {}", status);
        }
        Ok(())
    }
}
