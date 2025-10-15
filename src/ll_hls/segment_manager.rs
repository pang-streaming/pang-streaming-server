use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::ll_hls::playlist_generator::{Part, Segment};

#[derive(Debug, Clone)]
pub struct SegmentInfo {
    pub file_path: PathBuf,
    pub duration: f64,
    pub size: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct LLHLSSegmentManager {
    segments: Arc<RwLock<HashMap<String, Vec<SegmentInfo>>>>,
    output_dir: PathBuf,
    config: crate::config::HlsConfig,
}

impl LLHLSSegmentManager {
    pub fn new(output_dir: String, config: crate::config::HlsConfig) -> Self {
        Self {
            segments: Arc::new(RwLock::new(HashMap::new())),
            output_dir: PathBuf::from(output_dir),
            config,
        }
    }

    pub async fn initialize_stream(&self, stream_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stream_dir = self.output_dir.join(stream_id);
        fs::create_dir_all(&stream_dir).await?;
        
        let mut segments = self.segments.write().await;
        segments.insert(stream_id.to_string(), Vec::new());
        
        Ok(())
    }

    pub async fn create_segment(
        &self,
        stream_id: &str,
        sequence_number: u64,
        data: &[u8],
    ) -> Result<Segment, Box<dyn std::error::Error + Send + Sync>> {
        let stream_dir = self.output_dir.join(stream_id);
        let segment_filename = format!("segment_{}.m4s", sequence_number);
        let segment_path = stream_dir.join(&segment_filename);
        
        // 세그먼트 파일 저장
        fs::write(&segment_path, data).await?;
        
        let metadata = fs::metadata(&segment_path).await?;
        let duration = self.config.segment_duration;
        
        // 파트 생성 (LL-HLS의 핵심 기능)
        let parts = self.create_parts(stream_id, sequence_number, data, duration).await?;
        
        let segment = Segment {
            sequence_number,
            duration,
            uri: segment_filename,
            parts,
            independent: true,
            program_date_time: Some(chrono::Utc::now()),
        };

        // 세그먼트 정보 저장
        let mut segments = self.segments.write().await;
        if let Some(stream_segments) = segments.get_mut(stream_id) {
            let segment_info = SegmentInfo {
                file_path: segment_path,
                duration,
                size: metadata.len(),
                created_at: chrono::Utc::now(),
            };
            
            stream_segments.push(segment_info);
            
            // 최대 세그먼트 수 제한
            if stream_segments.len() > self.config.max_segments as usize {
                let old_segment = stream_segments.remove(0);
                let _ = fs::remove_file(old_segment.file_path).await;
            }
        }

        Ok(segment)
    }

    async fn create_parts(
        &self,
        stream_id: &str,
        sequence_number: u64,
        data: &[u8],
        total_duration: f64,
    ) -> Result<Vec<Part>, Box<dyn std::error::Error + Send + Sync>> {
        let part_duration = self.config.part_duration;
        let num_parts = (total_duration / part_duration).ceil() as usize;
        let part_size = data.len() / num_parts;
        
        let mut parts = Vec::new();
        
        for i in 0..num_parts {
            let start = i * part_size;
            let end = if i == num_parts - 1 {
                data.len()
            } else {
                (i + 1) * part_size
            };
            
            let part_data = &data[start..end];
            let part_filename = format!("part_{}_{}.m4s", sequence_number, i);
            let part_path = self.output_dir.join(stream_id).join(&part_filename);
            
            fs::write(&part_path, part_data).await?;
            
            let part = Part {
                duration: part_duration,
                uri: part_filename,
                independent: i == 0, // 첫 번째 파트만 independent
            };
            
            parts.push(part);
        }
        
        Ok(parts)
    }

    pub async fn cleanup_old_segments(&self, stream_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut segments = self.segments.write().await;
        if let Some(stream_segments) = segments.get_mut(stream_id) {
            let cutoff_time = chrono::Utc::now() - chrono::Duration::seconds(30);
            
            stream_segments.retain(|segment_info| {
                if segment_info.created_at < cutoff_time {
                    let _ = std::fs::remove_file(&segment_info.file_path);
                    false
                } else {
                    true
                }
            });
        }
        
        Ok(())
    }

    pub async fn get_segment_path(&self, stream_id: &str, filename: &str) -> Option<PathBuf> {
        let path = self.output_dir.join(stream_id).join(filename);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub async fn remove_stream(&self, stream_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stream_dir = self.output_dir.join(stream_id);
        if stream_dir.exists() {
            fs::remove_dir_all(&stream_dir).await?;
        }
        
        let mut segments = self.segments.write().await;
        segments.remove(stream_id);
        
        Ok(())
    }
}
