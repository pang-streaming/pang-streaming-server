use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use crate::monitoring::MetricsCollector;

#[derive(Debug, Clone)]
pub struct LatencyMeasurement {
    pub timestamp: DateTime<Utc>,
    pub latency_ms: f64,
    pub segment_sequence: u64,
    pub part_sequence: Option<u64>,
}

pub struct LatencyMonitor {
    measurements: Arc<RwLock<HashMap<String, Vec<LatencyMeasurement>>>>,
    metrics_collector: Arc<MetricsCollector>,
    target_latency_ms: f64,
}

impl LatencyMonitor {
    pub fn new(metrics_collector: Arc<MetricsCollector>, target_latency_ms: f64) -> Self {
        Self {
            measurements: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
            target_latency_ms,
        }
    }

    pub async fn record_segment_latency(
        &self,
        stream_id: &str,
        segment_sequence: u64,
        latency_ms: f64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let measurement = LatencyMeasurement {
            timestamp: Utc::now(),
            latency_ms,
            segment_sequence,
            part_sequence: None,
        };

        let mut measurements = self.measurements.write().await;
        if let Some(stream_measurements) = measurements.get_mut(stream_id) {
            stream_measurements.push(measurement);
            
            // 최대 100개의 측정값만 유지
            if stream_measurements.len() > 100 {
                stream_measurements.remove(0);
            }
        } else {
            measurements.insert(stream_id.to_string(), vec![measurement]);
        }

        // 메트릭 수집기에 지연시간 기록
        self.metrics_collector.record_latency(stream_id, latency_ms).await?;
        
        Ok(())
    }

    pub async fn record_part_latency(
        &self,
        stream_id: &str,
        segment_sequence: u64,
        part_sequence: u64,
        latency_ms: f64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let measurement = LatencyMeasurement {
            timestamp: Utc::now(),
            latency_ms,
            segment_sequence,
            part_sequence: Some(part_sequence),
        };

        let mut measurements = self.measurements.write().await;
        if let Some(stream_measurements) = measurements.get_mut(stream_id) {
            stream_measurements.push(measurement);
            
            // 최대 100개의 측정값만 유지
            if stream_measurements.len() > 100 {
                stream_measurements.remove(0);
            }
        } else {
            measurements.insert(stream_id.to_string(), vec![measurement]);
        }
        
        Ok(())
    }

    pub async fn get_average_latency(&self, stream_id: &str, window_seconds: u64) -> Option<f64> {
        let measurements = self.measurements.read().await;
        if let Some(stream_measurements) = measurements.get(stream_id) {
            let cutoff_time = Utc::now() - chrono::Duration::seconds(window_seconds as i64);
            let recent_measurements: Vec<&LatencyMeasurement> = stream_measurements
                .iter()
                .filter(|m| m.timestamp > cutoff_time)
                .collect();

            if !recent_measurements.is_empty() {
                let total_latency: f64 = recent_measurements.iter().map(|m| m.latency_ms).sum();
                Some(total_latency / recent_measurements.len() as f64)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn is_latency_acceptable(&self, stream_id: &str) -> bool {
        if let Some(avg_latency) = self.get_average_latency(stream_id, 10).await {
            avg_latency <= self.target_latency_ms
        } else {
            true // 측정값이 없으면 허용 가능한 것으로 간주
        }
    }

    pub async fn get_latency_trend(&self, stream_id: &str) -> LatencyTrend {
        let measurements = self.measurements.read().await;
        if let Some(stream_measurements) = measurements.get(stream_id) {
            if stream_measurements.len() < 2 {
                return LatencyTrend::Stable;
            }

            let recent = &stream_measurements[stream_measurements.len() - 5..];
            let older = &stream_measurements[stream_measurements.len() - 10..stream_measurements.len() - 5];

            if recent.is_empty() || older.is_empty() {
                return LatencyTrend::Stable;
            }

            let recent_avg: f64 = recent.iter().map(|m| m.latency_ms).sum::<f64>() / recent.len() as f64;
            let older_avg: f64 = older.iter().map(|m| m.latency_ms).sum::<f64>() / older.len() as f64;

            let change_percent = ((recent_avg - older_avg) / older_avg) * 100.0;

            if change_percent > 10.0 {
                LatencyTrend::Increasing
            } else if change_percent < -10.0 {
                LatencyTrend::Decreasing
            } else {
                LatencyTrend::Stable
            }
        } else {
            LatencyTrend::Stable
        }
    }

    pub async fn get_optimization_suggestions(&self, stream_id: &str) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();
        
        if let Some(avg_latency) = self.get_average_latency(stream_id, 30).await {
            if avg_latency > self.target_latency_ms * 1.5 {
                suggestions.push(OptimizationSuggestion::ReduceSegmentDuration);
            }
            
            if avg_latency > self.target_latency_ms * 2.0 {
                suggestions.push(OptimizationSuggestion::ReducePartDuration);
            }
            
            if avg_latency > self.target_latency_ms * 3.0 {
                suggestions.push(OptimizationSuggestion::EnableServerPush);
            }
        }

        let trend = self.get_latency_trend(stream_id).await;
        match trend {
            LatencyTrend::Increasing => {
                suggestions.push(OptimizationSuggestion::CheckNetworkConditions);
            }
            LatencyTrend::Decreasing => {
                suggestions.push(OptimizationSuggestion::LatencyImproving);
            }
            LatencyTrend::Stable => {
                // 안정적인 상태
            }
        }

        suggestions
    }

    pub async fn remove_stream_measurements(&self, stream_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut measurements = self.measurements.write().await;
        measurements.remove(stream_id);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LatencyTrend {
    Increasing,
    Decreasing,
    Stable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationSuggestion {
    ReduceSegmentDuration,
    ReducePartDuration,
    EnableServerPush,
    CheckNetworkConditions,
    LatencyImproving,
}

impl std::fmt::Display for OptimizationSuggestion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizationSuggestion::ReduceSegmentDuration => {
                write!(f, "세그먼트 지속시간을 줄이는 것을 고려하세요")
            }
            OptimizationSuggestion::ReducePartDuration => {
                write!(f, "파트 지속시간을 줄이는 것을 고려하세요")
            }
            OptimizationSuggestion::EnableServerPush => {
                write!(f, "서버 푸시를 활성화하는 것을 고려하세요")
            }
            OptimizationSuggestion::CheckNetworkConditions => {
                write!(f, "네트워크 상태를 확인하세요")
            }
            OptimizationSuggestion::LatencyImproving => {
                write!(f, "지연시간이 개선되고 있습니다")
            }
        }
    }
}
