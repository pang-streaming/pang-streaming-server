use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header, HeaderMap, HeaderValue, Response},
    response::IntoResponse,
    routing::get,
};

use tokio::fs::File;

use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use tokio_util::io::ReaderStream;
use tower_http::cors::CorsLayer;
use crate::transform_layer::hls_convertor::HlsConvertor;
use crate::monitoring::{MetricsCollector, LatencyMonitor};

pub struct M3U8Server {}

impl M3U8Server {
    pub fn new() -> Self {
        Self {}
    }
}

async fn get_master_playlist(
    Path(stream_key): Path<String>,
    State((hls_convertor, _, _)): State<(Arc<HlsConvertor>, Arc<MetricsCollector>, Arc<LatencyMonitor>)>,
) -> Result<impl IntoResponse, StatusCode> {
    let config = crate::config::get_config();
    
    if config.adaptive_bitrate.enabled {
        // 적응형 비트레이트 마스터 플레이리스트 생성
        match hls_convertor.get_playlist_generator()
            .generate_master_playlist(&stream_key, &config.adaptive_bitrate.variants)
            .await 
        {
            Ok(playlist_content) => {
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.apple.mpegurl"));
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
                
                Ok((headers, playlist_content))
            }
            Err(_) => Err(StatusCode::NOT_FOUND)
        }
    } else {
        // 단일 비트레이트 마스터 플레이리스트
        let master_playlist = format!(
            "#EXTM3U\n\
             #EXT-X-VERSION:9\n\
             #EXT-X-INDEPENDENT-SEGMENTS\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1400000,RESOLUTION=1280x720,CODECS=\"avc1.64001f,mp4a.40.2\"\n\
             {}/playlist.m3u8\n",
            stream_key
        );

        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.apple.mpegurl"));
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));

        Ok((headers, master_playlist))
    }
}

async fn get_segment_playlist(
    Path(stream_key): Path<String>,
    State((hls_convertor, _, _)): State<(Arc<HlsConvertor>, Arc<MetricsCollector>, Arc<LatencyMonitor>)>,
) -> Result<impl IntoResponse, StatusCode> {
    // 실제 플레이리스트 파일을 직접 읽기
    let playlist_path = format!("hls_output/{}/playlist.m3u8", stream_key);
    
    match tokio::fs::read_to_string(&playlist_path).await {
        Ok(playlist_content) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.apple.mpegurl"));
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
            
            // 프리로드 힌트 헤더 추가
            if let Some(hint_tag) = hls_convertor.get_preload_hint_manager()
                .generate_preload_hint_tag(&stream_key)
                .await 
            {
                if let Some(hint_uri) = hint_tag.split("URI=").nth(1) {
                    headers.insert(header::LINK, HeaderValue::from_str(&format!("<{}>; rel=preload", hint_uri)).unwrap());
                }
            }
            
            Ok((headers, playlist_content))
        }
        Err(_) => {
            // 기본 LL-HLS 플레이리스트
            let default_playlist = "#EXTM3U\n\
                 #EXT-X-VERSION:9\n\
                 #EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK=0.1,CAN-SKIP-UNTIL=0\n\
                 #EXT-X-TARGETDURATION:1\n\
                 #EXT-X-MEDIA-SEQUENCE:0\n\
                 #EXT-X-PLAYLIST-TYPE:LIVE\n".to_string();
            
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.apple.mpegurl"));
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
            
            Ok((headers, default_playlist))
        }
    }
}

async fn get_segment_file(
    Path((stream_key, segment)): Path<(String, String)>,
    State((hls_convertor, _, _)): State<(Arc<HlsConvertor>, Arc<MetricsCollector>, Arc<LatencyMonitor>)>,
) -> Result<impl IntoResponse, StatusCode> {
    let allowed_extensions = [".ts", ".m4s", ".mp4"];
    if !allowed_extensions.iter().any(|ext| segment.ends_with(ext)) {
        return Err(StatusCode::NOT_FOUND);
    }

    // 먼저 서버 푸시에서 리소스 확인
    if let Ok(response) = hls_convertor.get_server_push()
        .get_resource(&stream_key, &segment)
        .await 
    {
        return Ok(response);
    }

    // 파일 시스템에서 직접 읽기
    let file_path = PathBuf::from("hls_output").join(&stream_key).join(&segment);

    let file = File::open(file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = if segment.ends_with(".m4s") {
        "video/iso.segment"
    } else if segment.ends_with(".mp4") {
        "video/mp4"
    } else {
        "video/mp2t"
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap());
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .unwrap();
    
    *response.headers_mut() = headers;
    Ok(response)
}


async fn get_init_mp4(
    Path(stream_key): Path<String>,
) -> Result<([(String, String); 1], Vec<u8>), StatusCode> {
    let file_path = format!("./hls_output/{}/init.mp4", stream_key);

    match fs::read(&file_path).await {
        Ok(data) => Ok((
            [(
                header::CONTENT_TYPE.as_str().to_string(),
                "video/mp4".to_string(),
            )],
            data,
        )),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_ts_segment(
    Path((stream_key, segment)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !segment.ends_with(".ts") {
        return Err(StatusCode::NOT_FOUND);
    }

    let file_path = PathBuf::from("hls_output").join(&stream_key).join(&segment);

    let file = File::open(file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(([(header::CONTENT_TYPE, "video/mp2t")], body))
}

// 메트릭 API 엔드포인트들
async fn get_metrics(
    State((_, metrics_collector, _)): State<(Arc<HlsConvertor>, Arc<MetricsCollector>, Arc<LatencyMonitor>)>,
) -> Result<impl IntoResponse, StatusCode> {
    let metrics_json = metrics_collector.export_metrics_json().await;
    
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    
    Ok((headers, metrics_json))
}

async fn get_stream_metrics(
    Path(stream_key): Path<String>,
    State((_, metrics_collector, _)): State<(Arc<HlsConvertor>, Arc<MetricsCollector>, Arc<LatencyMonitor>)>,
) -> Result<impl IntoResponse, StatusCode> {
    match metrics_collector.get_stream_metrics(&stream_key).await {
        Some(metrics) => {
            let metrics_json = serde_json::to_string(&metrics).unwrap_or_else(|_| "{}".to_string());
            
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
            
            Ok((headers, metrics_json))
        }
        None => Err(StatusCode::NOT_FOUND)
    }
}

async fn get_latency_analysis(
    Path(stream_key): Path<String>,
    State((_, _, latency_monitor)): State<(Arc<HlsConvertor>, Arc<MetricsCollector>, Arc<LatencyMonitor>)>,
) -> Result<impl IntoResponse, StatusCode> {
    let avg_latency = latency_monitor.get_average_latency(&stream_key, 30).await;
    let trend = latency_monitor.get_latency_trend(&stream_key).await;
    let suggestions = latency_monitor.get_optimization_suggestions(&stream_key).await;
    
    let analysis = serde_json::json!({
        "stream_id": stream_key,
        "average_latency_ms": avg_latency,
        "trend": format!("{:?}", trend),
        "suggestions": suggestions.iter().map(|s| s.to_string()).collect::<Vec<String>>(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    
    Ok((headers, analysis.to_string()))
}

pub async fn start_m3u8_server(
    hls_convertor: Arc<HlsConvertor>,
    metrics_collector: Arc<MetricsCollector>,
    latency_monitor: Arc<LatencyMonitor>,
) -> Result<(), Box<dyn std::error::Error>> {
    let server = Arc::new(M3U8Server::new());

    let app = Router::new()
        // HLS 엔드포인트들
        .route("/hls/{stream_key}/master.m3u8", get(get_master_playlist))
        .route("/hls/{stream_key}/playlist.m3u8", get(get_segment_playlist))
        .route("/hls/{stream_key}/init.mp4", get(get_init_mp4))
        .route("/hls/{stream_key}/{segment}", get(get_segment_file))
        // 메트릭 API 엔드포인트들
        .route("/metrics", get(get_metrics))
        .route("/metrics/stream/{stream_key}", get(get_stream_metrics))
        .route("/metrics/latency/{stream_key}", get(get_latency_analysis))
        .layer(CorsLayer::permissive())
        .with_state((hls_convertor, metrics_collector, latency_monitor));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await?;
    axum::serve(listener, app).await?;
    Ok(())
}


pub fn start_m3u8_server_background(
    hls_convertor: Arc<HlsConvertor>,
    metrics_collector: Arc<MetricsCollector>,
    latency_monitor: Arc<LatencyMonitor>,
) {
    tokio::spawn(async move {
        if let Err(e) = start_m3u8_server(hls_convertor, metrics_collector, latency_monitor).await {
            eprintln!("Web server error: {}", e);
        }
    });
}
