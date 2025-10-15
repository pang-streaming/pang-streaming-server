use std::sync::Arc;
use scuffle_rtmp::ServerSession;
use reqwest::Client;
use tokio::net::TcpListener;
mod config;
mod handler;
mod m3u8_server;
mod authentication_layer;
mod utils;
mod transform_layer;
mod ll_hls;
mod monitoring;

use handler::Handler;
use m3u8_server::start_m3u8_server_background;
use crate::transform_layer::hls_convertor::HlsConvertor;
use crate::monitoring::{MetricsCollector, LatencyMonitor};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = config::get_config();
    let client = Arc::new(Client::new());
    let mut hls_convertor = HlsConvertor::new(format!("{}", config.hls.save_dir))?;
    
    // 메트릭 수집기와 지연시간 모니터 초기화
    let metrics_collector = Arc::new(MetricsCollector::new());
    let latency_monitor = Arc::new(LatencyMonitor::new(
        Arc::clone(&metrics_collector),
        config.hls.target_latency * 1000.0, // 초를 밀리초로 변환
    ));
    
    // HLS 변환기에 메트릭 수집기와 지연시간 모니터 설정
    hls_convertor.set_metrics_collector(Arc::clone(&metrics_collector));
    hls_convertor.set_latency_monitor(Arc::clone(&latency_monitor));
    
    let hls_convertor = Arc::new(hls_convertor);
    
    // LL-HLS M3U8 서버 시작
    start_m3u8_server_background(
        Arc::clone(&hls_convertor),
        Arc::clone(&metrics_collector),
        Arc::clone(&latency_monitor),
    );
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    let listener = TcpListener::bind(format!("[::]:{}", config.server.port)).await?;
    println!("🚀 LL-HLS RTMP Server listening on [::]:{}", config.server.port);
    println!("📺 HLS Playlist available at: http://localhost:8081/hls/{{stream_key}}/playlist.m3u8");
    println!("📊 Metrics available at: http://localhost:8081/metrics");
    println!("🎯 Target latency: {}s", config.hls.target_latency);
    println!("⚡ Segment duration: {}s", config.hls.segment_duration);
    println!("🔧 Part duration: {}s", config.hls.part_duration);
    println!("🔄 Adaptive bitrate: {}", if config.adaptive_bitrate.enabled { "enabled" } else { "disabled" });

    while let Ok((stream, addr)) = listener.accept().await {
        println!("New connection from: {}", addr);
        let hls_convertor_clone = Arc::clone(&hls_convertor);
        let client_clone = Arc::clone(&client);
        tokio::spawn(async move {
            let handler = match Handler::new(hls_convertor_clone, client_clone) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Failed to create handler for {}: {}", addr, e);
                    return;
                }
            };

            let session = ServerSession::new(stream, handler);
            if let Err(err) = session.run().await {
                eprintln!("Session error from {}: {:?}", addr, err);
            }
        });
    }
    Ok(())
}