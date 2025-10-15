use std::collections::HashMap;
use std::error::Error;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::utils::log_error::LogError;

pub struct HlsConvertor {
    pipelines: Arc<Mutex<HashMap<u32, FfmpegPipeline>>>,
    output_dir: String,
    segment_delay: u32,
}

pub struct FfmpegPipeline {
    pub stdin: std::process::ChildStdin,
}

impl HlsConvertor {
    pub fn new(output_dir: String) -> Result<Self, Box<dyn Error>> {
        let config = crate::config::get_config();
        let segment_delay = config.server.segment_delay;
        std::fs::create_dir_all(&output_dir)
            .log_error("Failed to create output directory: ");

        Ok(Self {
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            output_dir,
            segment_delay,
        })
    }

    pub fn get_pipelines(&self) -> Arc<Mutex<HashMap<u32, FfmpegPipeline>>> {
        self.pipelines.clone()
    }

    pub fn start_hls_conversion(
        &self,
        stream_id: u32,
        stream_name: &str,
        stream_host: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let output_path = format!("{}/{}", self.output_dir, stream_name);

        if !self.output_dir.starts_with("s3://") {
            std::fs::create_dir_all(&output_path)?;
        }

        let output_playlist = format!("{}/playlist.m3u8", output_path);
        let init_file = format!("{}/init.mp4", output_path);
        let segment_filename_pattern = format!("{}/segment_%d.m4s", output_path);

        let mut child = Command::new("ffmpeg")
            .args([
                "-i", "pipe:0",
                "-c:v", "copy",
                "-c:a", "copy",
                "-preset", "veryfast",
                "-f", "hls",
                "-hls_time", "0.5",
                "-hls_list_size", "0",
                "-hls_flags", "delete_segments+program_date_time+temp_file+independent_segments",
                "-hls_segment_type", "fmp4",
                "-hls_fmp4_init_filename", "init.mp4",
                "-hls_segment_filename", &segment_filename_pattern,
                &output_playlist,
            ])
            .stdin(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin")?;

        let mut pipelines = self.pipelines.lock().unwrap();
        pipelines.insert(stream_id, FfmpegPipeline { stdin });

        println!("HLS conversion started for stream {} (key: {})", stream_id, stream_name);
        println!("Playlist available at: {}/playlist.m3u8", output_path);

        Ok(())
    }

    pub fn stop_hls_conversion(&self, stream_id: u32) {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(_pipeline) = pipelines.remove(&stream_id) {
            // The ffmpeg process will exit automatically when stdin is closed.
            println!("FFmpeg HLS conversion stopped for stream {}", stream_id);
        }
    }
}