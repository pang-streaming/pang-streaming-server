use std::sync::Arc;
use scuffle_rtmp::session::server::{ServerSession, ServerSessionError, SessionHandler, SessionData};
use crate::business_layer::streaming::hls_convertor::HlsConvertor;

/// RTMP 세션 핸들러
pub struct RtmpSessionHandler {
    hls_convertor: Arc<HlsConvertor>,
}

impl RtmpSessionHandler {
    /// 새로운 RTMP 세션 핸들러 생성
    pub fn new(hls_convertor: Arc<HlsConvertor>) -> Self {
        Self { hls_convertor }
    }
}

impl SessionHandler for RtmpSessionHandler {
    async fn on_publish(
        &mut self,
        stream_id: u32,
        _app_name: &str,
        stream_key: &str,
    ) -> Result<(), ServerSessionError> {
        println!("📡 RTMP publish request: stream_id={}, stream_key={}", stream_id, stream_key);

        // 스트림 데이터 크기 검증
        if stream_id == 0 {
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        // let authed_stream_id: &str = &authenticate_and_get_stream_id(stream_key, &self.http_client).await?;
        let authed_stream_id = stream_key;

        if let Err(e) = self.hls_convertor.start_hls_conversion(stream_id, authed_stream_id).await {
            eprintln!("Failed to start HLS conversion: {}", e);
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        let mut header = Vec::new();
        header.extend_from_slice(b"FLV"); // Signature
        header.push(1); // Version
        header.push(0x05); // Flags (audio + video)
        header.extend_from_slice(&9u32.to_be_bytes()); // DataOffset
        header.extend_from_slice(&0u32.to_be_bytes()); // PreviousTagSize0
        // FLV 헤더를 스트림 데이터로 처리
        if let Err(e) = self.hls_convertor.process_stream_data(stream_id, &header).await {
            eprintln!("Failed to process FLV header: {}", e);
        }

        Ok(())
    }

    async fn on_unpublish(&mut self, stream_id: u32) -> Result<(), ServerSessionError> {
        if let Err(e) = self.hls_convertor.stop_hls_conversion(stream_id, "unknown").await {
            eprintln!("Failed to stop HLS conversion: {}", e);
        }
        Ok(())
    }

    async fn on_data(
        &mut self,
        stream_id: u32,
        data: SessionData,
    ) -> Result<(), ServerSessionError> {
        let (tag_type, timestamp, payload) = match data {
            SessionData::Video { timestamp, data } => (9, timestamp, data),
            SessionData::Audio { timestamp, data } => (8, timestamp, data),
            SessionData::Amf0 { timestamp, data } => (18, timestamp, data),
        };

        let data_size = payload.len() as u32;
        let mut flv_tag = Vec::new();
        flv_tag.push(tag_type); // TagType
        flv_tag.extend_from_slice(&(data_size.to_be_bytes()[1..])); // DataSize
        flv_tag.extend_from_slice(&(timestamp.to_be_bytes()[1..])); // Timestamp
        flv_tag.push((timestamp >> 24) as u8); // TimestampExtended
        flv_tag.extend_from_slice(&[0, 0, 0]); // StreamID
        flv_tag.extend_from_slice(&payload);
        flv_tag.extend_from_slice(&(data_size + 11).to_be_bytes()); // PreviousTagSize

        if let Err(e) = self.hls_convertor.process_stream_data(stream_id, &flv_tag).await {
            // 파이프라인이 깨진 경우 더 이상 데이터를 전송하지 않음
            if e.to_string().contains("Pipeline broken") {
                eprintln!("Pipeline broken for stream {}, stopping data processing", stream_id);
                return Ok(()); // 더 이상 데이터 처리 중단
            }
            eprintln!("Failed to process stream data: {}", e);
        }

        Ok(())
    }
}
