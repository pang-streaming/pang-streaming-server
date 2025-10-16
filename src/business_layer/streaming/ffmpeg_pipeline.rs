use std::process::{Command, ChildStdin, Stdio};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::io::Write;
use crate::config::Config;

/// FFmpeg 파이프라인 정보
#[derive(Debug)]
pub struct FfmpegPipeline {
    pub stdin: ChildStdin,
    pub stream_id: u32,
}

/// FFmpeg 파이프라인 관리자
pub struct FfmpegPipelineManager {
    pipelines: Arc<Mutex<HashMap<u32, FfmpegPipeline>>>,
    config: Config,
}

impl FfmpegPipelineManager {
    /// 새로운 FFmpeg 파이프라인 관리자 생성
    pub fn new(config: Config) -> Self {
        Self {
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// 새로운 FFmpeg 파이프라인 시작
    pub async fn start_pipeline(
        &self,
        stream_id: u32,
        stream_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let output_dir = format!("hls_output/{}", stream_name);
        
        // 출력 디렉토리 생성
        tokio::fs::create_dir_all(&output_dir).await?;

        // FFmpeg 명령어 구성 (LL-HLS 최적화)
        let segment_filename_pattern = format!("{}/segment_%d.m4s", output_dir);
        let hls_base_url = format!("https://{}.s3.{}.amazonaws.com/hls_output/{}/", &self.config.s3.bucket, &self.config.s3.region, stream_name);
        let playlist_path = format!("{}/playlist.m3u8", output_dir);
        

        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-i", "pipe:0",
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-tune", "zerolatency",
            "-g", "30", // GOP 크기 (키프레임 간격)
            "-keyint_min", "30",
            "-sc_threshold", "0",
            "-c:a", "aac",
            "-b:a", "128k",
            "-ar", "44100",
            "-ac", "2",
            "-f", "hls",
            "-hls_time", "0.5",
            "-hls_list_size", "0",
            "-hls_flags", "delete_segments+program_date_time+temp_file+independent_segments+split_by_time",
            "-hls_segment_type", "fmp4",
            "-hls_fmp4_init_filename", "init.mp4",
            "-hls_segment_filename", &segment_filename_pattern,
            "-hls_playlist_type", "event",
            "-hls_allow_cache", "0",
            "-hls_start_number_source", "datetime",
            "-hls_base_url", &hls_base_url,
            &playlist_path,
        ]);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or("Failed to get stdin")?;
        let mut stderr = child.stderr.take().ok_or("Failed to get stderr")?;

        let pipeline = FfmpegPipeline {
            stdin,
            stream_id,
        };

        // 파이프라인 저장
        {
            let mut pipelines = self.pipelines.lock().unwrap();
            pipelines.insert(stream_id, pipeline);
        }

        // FFmpeg stderr 모니터링 (백그라운드에서)
        let stream_id_clone = stream_id;
        tokio::spawn(async move {
            use std::io::Read;
            let mut buffer = [0; 1024];
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buffer[..n]);
                        if !output.trim().is_empty() {
                            println!("FFmpeg[{}]: {}", stream_id_clone, output.trim());
                        }
                    }
                    Err(e) => {
                        println!("FFmpeg[{}]: stderr read error: {}", stream_id_clone, e);
                        break;
                    }
                }
            }
            println!("FFmpeg[{}]: stderr monitoring ended", stream_id_clone);
        });

        println!("🎬 FFmpeg pipeline started for stream {} (key: {})", stream_id, stream_name);
        println!("📁 Playlist available at: {}/playlist.m3u8", output_dir);

        Ok(())
    }

    /// 파이프라인에 데이터 전송
    pub fn send_data(&self, stream_id: u32, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline) = pipelines.get_mut(&stream_id) {
            match pipeline.stdin.write_all(data) {
                Ok(_) => {
                    if let Err(e) = pipeline.stdin.flush() {
                        // 파이프가 깨진 경우 파이프라인 제거
                        pipelines.remove(&stream_id);
                        return Err(format!("Pipeline broken for stream {}: {}", stream_id, e).into());
                    }
                }
                Err(e) => {
                    // 파이프가 깨진 경우 파이프라인 제거
                    pipelines.remove(&stream_id);
                    return Err(format!("Pipeline broken for stream {}: {}", stream_id, e).into());
                }
            }
        }
        Ok(())
    }

    /// 파이프라인 종료
    pub fn stop_pipeline(&self, stream_id: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut pipelines = self.pipelines.lock().unwrap();
        if pipelines.remove(&stream_id).is_some() {
            println!("🛑 FFmpeg pipeline stopped for stream {}", stream_id);
        }
        Ok(())
    }

    /// 모든 파이프라인 종료
    pub fn stop_all_pipelines(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut pipelines = self.pipelines.lock().unwrap();
        pipelines.clear();
        println!("🛑 All FFmpeg pipelines stopped");
        Ok(())
    }

    /// 파이프라인 존재 여부 확인
    pub fn has_pipeline(&self, stream_id: u32) -> bool {
        let pipelines = self.pipelines.lock().unwrap();
        pipelines.contains_key(&stream_id)
    }

    /// 활성 파이프라인 수 조회
    pub fn active_pipeline_count(&self) -> usize {
        let pipelines = self.pipelines.lock().unwrap();
        pipelines.len()
    }
}
