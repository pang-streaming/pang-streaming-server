use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub sequence_number: u64,
    pub duration: f64,
    pub uri: String,
    pub parts: Vec<Part>,
    pub independent: bool,
    pub program_date_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub duration: f64,
    pub uri: String,
    pub independent: bool,
}

#[derive(Debug, Clone)]
pub struct StreamState {
    pub stream_id: String,
    pub sequence_number: u64,
    pub target_duration: f64,
    pub segments: Vec<Segment>,
    pub last_updated: DateTime<Utc>,
    pub playlist_type: PlaylistType,
}

#[derive(Debug, Clone)]
pub enum PlaylistType {
    Event,
    Live,
}

pub struct LLHLSPlaylistGenerator {
    streams: Arc<RwLock<HashMap<String, StreamState>>>,
    config: crate::config::HlsConfig,
}

impl LLHLSPlaylistGenerator {
    pub fn new(config: crate::config::HlsConfig) -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn create_stream(&self, stream_id: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut streams = self.streams.write().await;
        streams.insert(
            stream_id.clone(),
            StreamState {
                stream_id: stream_id.clone(),
                sequence_number: 0,
                target_duration: self.config.segment_duration,
                segments: Vec::new(),
                last_updated: Utc::now(),
                playlist_type: PlaylistType::Live,
            },
        );
        Ok(())
    }

    pub async fn add_segment(
        &self,
        stream_id: &str,
        segment_uri: String,
        duration: f64,
        parts: Vec<Part>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut streams = self.streams.write().await;
        if let Some(stream) = streams.get_mut(stream_id) {
            let segment = Segment {
                sequence_number: stream.sequence_number,
                duration,
                uri: segment_uri,
                parts,
                independent: true,
                program_date_time: Some(Utc::now()),
            };

            stream.segments.push(segment);
            stream.sequence_number += 1;
            stream.last_updated = Utc::now();

            // 최대 세그먼트 수 제한
            if stream.segments.len() > self.config.max_segments as usize {
                stream.segments.remove(0);
            }
        }
        Ok(())
    }

    pub async fn generate_playlist(&self, stream_id: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let streams = self.streams.read().await;
        if let Some(stream) = streams.get(stream_id) {
            let mut playlist = String::new();
            
            // LL-HLS 헤더
            playlist.push_str("#EXTM3U\n");
            playlist.push_str("#EXT-X-VERSION:9\n");
            playlist.push_str("#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK=0.1,CAN-SKIP-UNTIL=0\n");
            playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", 
                (stream.target_duration + 0.5).ceil() as u32));
            playlist.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", stream.sequence_number - stream.segments.len() as u64));
            
            // LL-HLS 특별 태그들
            if self.config.enable_preload_hint {
                playlist.push_str("#EXT-X-PRELOAD-HINT:TYPE=PART,URI=part_0.m4s\n");
            }

            // 세그먼트들
            for segment in &stream.segments {
                if segment.independent {
                    playlist.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");
                }
                
                if let Some(pdt) = segment.program_date_time {
                    playlist.push_str(&format!("#EXT-X-PROGRAM-DATE-TIME:{}\n", pdt.to_rfc3339()));
                }

                // 파트들
                for part in &segment.parts {
                    let part_tag = if part.independent {
                        format!("#EXT-X-PART:DURATION={},URI={},INDEPENDENT=YES\n", 
                            part.duration, part.uri)
                    } else {
                        format!("#EXT-X-PART:DURATION={},URI={}\n", 
                            part.duration, part.uri)
                    };
                    playlist.push_str(&part_tag);
                }

                // 세그먼트
                playlist.push_str(&format!("#EXTINF:{},\n", segment.duration));
                playlist.push_str(&format!("{}\n", segment.uri));
            }

            Ok(playlist)
        } else {
            Err("Stream not found".into())
        }
    }

    pub async fn generate_master_playlist(
        &self,
        stream_id: &str,
        variants: &[crate::config::BitrateVariant],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut playlist = String::new();
        
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:9\n");
        playlist.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");

        for variant in variants {
            let stream_inf = format!(
                "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={},CODECS=\"avc1.64001f,mp4a.40.2\"\n",
                variant.bandwidth, variant.resolution
            );
            playlist.push_str(&stream_inf);
            playlist.push_str(&format!("{}/playlist.m3u8\n", variant.name));
        }

        Ok(playlist)
    }

    pub async fn remove_stream(&self, stream_id: &str) {
        let mut streams = self.streams.write().await;
        streams.remove(stream_id);
    }
}
