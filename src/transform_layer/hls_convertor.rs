use std::collections::HashMap;
use std::error::Error;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::utils::log_error::LogError;
use crate::ll_hls::{
    LLHLSPlaylistGenerator, LLHLSSegmentManager, LLHLSServerPush, LLHLSPreloadHintManager,
    playlist_generator::Part,
};
use crate::monitoring::{MetricsCollector, LatencyMonitor};

pub struct HlsConvertor {
    pipelines: Arc<Mutex<HashMap<u32, FfmpegPipeline>>>,
    output_dir: String,
    segment_delay: u32,
    playlist_generator: Arc<LLHLSPlaylistGenerator>,
    segment_manager: Arc<LLHLSSegmentManager>,
    server_push: Arc<LLHLSServerPush>,
    preload_hint_manager: Arc<LLHLSPreloadHintManager>,
    active_streams: Arc<RwLock<HashMap<u32, String>>>,
    metrics_collector: Option<Arc<MetricsCollector>>,
    latency_monitor: Option<Arc<LatencyMonitor>>,
}

pub struct FfmpegPipeline {
    pub stdin: std::process::ChildStdin,
    pub stream_id: u32,
    pub stream_name: String,
}

impl HlsConvertor {
    pub fn new(output_dir: String) -> Result<Self, Box<dyn Error>> {
        let config = crate::config::get_config();
        let segment_delay = config.server.segment_delay;
        std::fs::create_dir_all(&output_dir)
            .log_error("Failed to create output directory: ");

        let playlist_generator = Arc::new(LLHLSPlaylistGenerator::new(config.hls.clone()));
        let segment_manager = Arc::new(LLHLSSegmentManager::new(output_dir.clone(), config.hls.clone()));
        let server_push = Arc::new(LLHLSServerPush::new());
        let preload_hint_manager = Arc::new(LLHLSPreloadHintManager::new());

        Ok(Self {
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            output_dir,
            segment_delay,
            playlist_generator,
            segment_manager,
            server_push,
            preload_hint_manager,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector: None,
            latency_monitor: None,
        })
    }

    pub fn get_pipelines(&self) -> Arc<Mutex<HashMap<u32, FfmpegPipeline>>> {
        self.pipelines.clone()
    }

    pub fn get_playlist_generator(&self) -> Arc<LLHLSPlaylistGenerator> {
        self.playlist_generator.clone()
    }

    pub fn get_segment_manager(&self) -> Arc<LLHLSSegmentManager> {
        self.segment_manager.clone()
    }

    pub fn get_server_push(&self) -> Arc<LLHLSServerPush> {
        self.server_push.clone()
    }

    pub fn get_preload_hint_manager(&self) -> Arc<LLHLSPreloadHintManager> {
        self.preload_hint_manager.clone()
    }

    pub fn set_metrics_collector(&mut self, metrics_collector: Arc<MetricsCollector>) {
        self.metrics_collector = Some(metrics_collector);
    }

    pub fn set_latency_monitor(&mut self, latency_monitor: Arc<LatencyMonitor>) {
        self.latency_monitor = Some(latency_monitor);
    }

    pub async fn start_hls_conversion(
        &self,
        stream_id: u32,
        stream_name: &str,
        stream_host: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let config = crate::config::get_config();
        let output_path = format!("{}/{}", self.output_dir, stream_name);

        if !self.output_dir.starts_with("s3://") {
            std::fs::create_dir_all(&output_path)?;
        }

        // LL-HLS 컴포넌트 초기화
        self.playlist_generator.create_stream(stream_name.to_string()).await?;
        self.segment_manager.initialize_stream(stream_name).await?;

        // 메트릭 수집기 초기화
        if let Some(metrics_collector) = &self.metrics_collector {
            let _ = metrics_collector.create_stream_metrics(stream_name.to_string()).await;
        }

        // 활성 스트림 등록
        {
            let mut active_streams = self.active_streams.write().await;
            active_streams.insert(stream_id, stream_name.to_string());
        }

        // LL-HLS 최적화된 FFmpeg 명령어
        let segment_duration_str = config.hls.segment_duration.to_string();
        let segment_filename_pattern = format!("{}/segment_%d.m4s", output_path);
        let hls_base_url = format!("http://localhost:8081/hls/{}/", stream_name);
        let playlist_path = format!("{}/playlist.m3u8", output_path);
        
        let mut ffmpeg_args = vec![
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
            "-hls_time", &segment_duration_str,
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
        ];

        // 적응형 비트레이트가 활성화된 경우 (현재는 단일 비트레이트만 지원)
        // TODO: 다중 비트레이트 지원을 위한 복잡한 FFmpeg 명령어 구현

        let mut child = Command::new("ffmpeg")
            .args(&ffmpeg_args)
            .stdin(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin")?;

        let mut pipelines = self.pipelines.lock().unwrap();
        pipelines.insert(stream_id, FfmpegPipeline { 
            stdin,
            stream_id,
            stream_name: stream_name.to_string(),
        });

        println!("LL-HLS conversion started for stream {} (key: {})", stream_id, stream_name);
        println!("Playlist available at: {}/playlist.m3u8", output_path);

        // 백그라운드에서 세그먼트 모니터링 시작
        let segment_manager = self.segment_manager.clone();
        let playlist_generator = self.playlist_generator.clone();
        let server_push = self.server_push.clone();
        let preload_hint_manager = self.preload_hint_manager.clone();
        let metrics_collector = self.metrics_collector.clone();
        let latency_monitor = self.latency_monitor.clone();
        let stream_name_clone = stream_name.to_string();
        
        tokio::spawn(async move {
            Self::monitor_segments(
                segment_manager,
                playlist_generator,
                server_push,
                preload_hint_manager,
                metrics_collector,
                latency_monitor,
                stream_name_clone,
            ).await;
        });

        Ok(())
    }

    async fn monitor_segments(
        segment_manager: Arc<LLHLSSegmentManager>,
        playlist_generator: Arc<LLHLSPlaylistGenerator>,
        server_push: Arc<LLHLSServerPush>,
        preload_hint_manager: Arc<LLHLSPreloadHintManager>,
        metrics_collector: Option<Arc<MetricsCollector>>,
        latency_monitor: Option<Arc<LatencyMonitor>>,
        stream_name: String,
    ) {
        let mut sequence_number = 0u64;
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            // 새로운 세그먼트 파일 확인
            let segment_filename = format!("segment_{}.m4s", sequence_number);
            if let Some(segment_path) = segment_manager.get_segment_path(&stream_name, &segment_filename).await {
                if let Ok(segment_data) = tokio::fs::read(&segment_path).await {
                    // 세그먼트 생성
                    if let Ok(segment) = segment_manager.create_segment(&stream_name, sequence_number, &segment_data).await {
                        // 메트릭 수집
                        if let Some(metrics_collector) = &metrics_collector {
                            let _ = metrics_collector.record_segment(&stream_name, segment.duration, segment_data.len() as u64).await;
                        }

                        // 지연시간 측정 (세그먼트 생성 시간 기준)
                        if let Some(latency_monitor) = &latency_monitor {
                            let latency_ms = (chrono::Utc::now() - segment.program_date_time.unwrap_or(chrono::Utc::now())).num_milliseconds() as f64;
                            let _ = latency_monitor.record_segment_latency(&stream_name, sequence_number, latency_ms).await;
                        }

                        // 플레이리스트에 세그먼트 추가
                        let _ = playlist_generator.add_segment(
                            &stream_name,
                            segment.uri.clone(),
                            segment.duration,
                            segment.parts.clone(),
                        ).await;

                        // 서버 푸시에 세그먼트 등록
                        let _ = server_push.push_segment(&stream_name, &segment.uri, segment_data.clone()).await;

                        // 파트들도 서버 푸시에 등록
                        for (part_index, part) in segment.parts.iter().enumerate() {
                            if let Ok(part_data) = tokio::fs::read(segment_path.parent().unwrap().join(&part.uri)).await {
                                let _ = server_push.push_segment(&stream_name, &part.uri, part_data).await;
                                
                                // 메트릭 수집 (파트)
                                if let Some(metrics_collector) = &metrics_collector {
                                    let _ = metrics_collector.record_part(&stream_name, part.duration).await;
                                }

                                // 지연시간 측정 (파트)
                                if let Some(latency_monitor) = &latency_monitor {
                                    let latency_ms = (chrono::Utc::now() - segment.program_date_time.unwrap_or(chrono::Utc::now())).num_milliseconds() as f64;
                                    let _ = latency_monitor.record_part_latency(&stream_name, sequence_number, part_index as u64, latency_ms).await;
                                }
                                
                                // 프리로드 힌트 추가
                                let _ = preload_hint_manager.add_hint(
                                    &stream_name,
                                    part.uri.clone(),
                                    crate::ll_hls::preload_hint::HintType::Part,
                                    Some(part.duration),
                                ).await;
                            }
                        }

                        // 플레이리스트 업데이트
                        if let Ok(playlist_content) = playlist_generator.generate_playlist(&stream_name).await {
                            let _ = server_push.push_playlist(&stream_name, playlist_content).await;
                        }

                        sequence_number += 1;
                    }
                }
            }
        }
    }

    pub fn stop_hls_conversion(&self, stream_id: u32) {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline) = pipelines.remove(&stream_id) {
            // The ffmpeg process will exit automatically when stdin is closed.
            println!("FFmpeg LL-HLS conversion stopped for stream {}", stream_id);
            
            // LL-HLS 컴포넌트 정리
            let stream_name = pipeline.stream_name;
            let playlist_generator = self.playlist_generator.clone();
            let segment_manager = self.segment_manager.clone();
            let server_push = self.server_push.clone();
            let preload_hint_manager = self.preload_hint_manager.clone();
            let active_streams = self.active_streams.clone();
            let metrics_collector = self.metrics_collector.clone();
            let latency_monitor = self.latency_monitor.clone();
            
            tokio::spawn(async move {
                let _ = playlist_generator.remove_stream(&stream_name).await;
                let _ = segment_manager.remove_stream(&stream_name).await;
                let _ = server_push.remove_stream_resources(&stream_name).await;
                let _ = preload_hint_manager.remove_stream_hints(&stream_name).await;
                
                // 메트릭 정리
                if let Some(metrics_collector) = metrics_collector {
                    let _ = metrics_collector.remove_stream_metrics(&stream_name).await;
                }
                if let Some(latency_monitor) = latency_monitor {
                    let _ = latency_monitor.remove_stream_measurements(&stream_name).await;
                }
                
                let mut streams = active_streams.write().await;
                streams.remove(&stream_id);
            });
        }
    }
}