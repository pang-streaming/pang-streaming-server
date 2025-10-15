use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamMetrics {
    pub stream_id: String,
    pub start_time: DateTime<Utc>,
    pub total_segments: u64,
    pub total_parts: u64,
    pub average_segment_duration: f64,
    pub average_part_duration: f64,
    pub total_bytes: u64,
    pub current_bitrate: u32,
    pub dropped_segments: u64,
    pub last_segment_time: Option<DateTime<Utc>>,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerMetrics {
    pub active_streams: u32,
    pub total_connections: u64,
    pub total_bytes_served: u64,
    pub average_latency_ms: f64,
    pub uptime_seconds: u64,
    pub start_time: DateTime<Utc>,
}

pub struct MetricsCollector {
    stream_metrics: Arc<RwLock<HashMap<String, StreamMetrics>>>,
    server_metrics: Arc<RwLock<ServerMetrics>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            stream_metrics: Arc::new(RwLock::new(HashMap::new())),
            server_metrics: Arc::new(RwLock::new(ServerMetrics {
                active_streams: 0,
                total_connections: 0,
                total_bytes_served: 0,
                average_latency_ms: 0.0,
                uptime_seconds: 0,
                start_time: Utc::now(),
            })),
        }
    }

    pub async fn create_stream_metrics(&self, stream_id: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut metrics = self.stream_metrics.write().await;
        metrics.insert(stream_id.clone(), StreamMetrics {
            stream_id: stream_id.clone(),
            start_time: Utc::now(),
            total_segments: 0,
            total_parts: 0,
            average_segment_duration: 0.0,
            average_part_duration: 0.0,
            total_bytes: 0,
            current_bitrate: 0,
            dropped_segments: 0,
            last_segment_time: None,
            latency_ms: 0.0,
        });

        // 서버 메트릭 업데이트
        let mut server_metrics = self.server_metrics.write().await;
        server_metrics.active_streams += 1;
        server_metrics.total_connections += 1;
        
        Ok(())
    }

    pub async fn record_segment(&self, stream_id: &str, duration: f64, size: u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut metrics = self.stream_metrics.write().await;
        if let Some(stream_metric) = metrics.get_mut(stream_id) {
            stream_metric.total_segments += 1;
            stream_metric.total_bytes += size;
            stream_metric.last_segment_time = Some(Utc::now());
            
            // 평균 세그먼트 지속시간 계산
            let total_duration = stream_metric.average_segment_duration * (stream_metric.total_segments - 1) as f64 + duration;
            stream_metric.average_segment_duration = total_duration / stream_metric.total_segments as f64;
            
            // 현재 비트레이트 계산 (초당 바이트)
            if let Some(last_time) = stream_metric.last_segment_time {
                let elapsed = (last_time - stream_metric.start_time).num_milliseconds() as f64 / 1000.0;
                if elapsed > 0.0 {
                    stream_metric.current_bitrate = (stream_metric.total_bytes as f64 / elapsed * 8.0) as u32;
                }
            }
        }
        Ok(())
    }

    pub async fn record_part(&self, stream_id: &str, duration: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut metrics = self.stream_metrics.write().await;
        if let Some(stream_metric) = metrics.get_mut(stream_id) {
            stream_metric.total_parts += 1;
            
            // 평균 파트 지속시간 계산
            let total_duration = stream_metric.average_part_duration * (stream_metric.total_parts - 1) as f64 + duration;
            stream_metric.average_part_duration = total_duration / stream_metric.total_parts as f64;
        }
        Ok(())
    }

    pub async fn record_latency(&self, stream_id: &str, latency_ms: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut metrics = self.stream_metrics.write().await;
        if let Some(stream_metric) = metrics.get_mut(stream_id) {
            stream_metric.latency_ms = latency_ms;
        }

        // 서버 평균 지연시간 업데이트
        let mut server_metrics = self.server_metrics.write().await;
        let total_latency = server_metrics.average_latency_ms * server_metrics.active_streams as f64 + latency_ms;
        server_metrics.average_latency_ms = total_latency / (server_metrics.active_streams + 1) as f64;
        
        Ok(())
    }

    pub async fn record_dropped_segment(&self, stream_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut metrics = self.stream_metrics.write().await;
        if let Some(stream_metric) = metrics.get_mut(stream_id) {
            stream_metric.dropped_segments += 1;
        }
        Ok(())
    }

    pub async fn remove_stream_metrics(&self, stream_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut metrics = self.stream_metrics.write().await;
        metrics.remove(stream_id);

        let mut server_metrics = self.server_metrics.write().await;
        if server_metrics.active_streams > 0 {
            server_metrics.active_streams -= 1;
        }
        Ok(())
    }

    pub async fn get_stream_metrics(&self, stream_id: &str) -> Option<StreamMetrics> {
        let metrics = self.stream_metrics.read().await;
        metrics.get(stream_id).cloned()
    }

    pub async fn get_server_metrics(&self) -> ServerMetrics {
        let mut server_metrics = self.server_metrics.read().await.clone();
        server_metrics.uptime_seconds = (Utc::now() - server_metrics.start_time).num_seconds() as u64;
        server_metrics
    }

    pub async fn get_all_stream_metrics(&self) -> HashMap<String, StreamMetrics> {
        self.stream_metrics.read().await.clone()
    }

    // 메트릭을 JSON으로 내보내기
    pub async fn export_metrics_json(&self) -> String {
        let server_metrics = self.get_server_metrics().await;
        let stream_metrics = self.get_all_stream_metrics().await;
        
        serde_json::json!({
            "server": {
                "active_streams": server_metrics.active_streams,
                "total_connections": server_metrics.total_connections,
                "total_bytes_served": server_metrics.total_bytes_served,
                "average_latency_ms": server_metrics.average_latency_ms,
                "uptime_seconds": server_metrics.uptime_seconds,
                "start_time": server_metrics.start_time.to_rfc3339()
            },
            "streams": stream_metrics
        }).to_string()
    }
}
