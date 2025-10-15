use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};
use std::sync::{Arc};
use std::io::Write;

use reqwest::Client;
use crate::authentication_layer::auth::authenticate_and_get_stream_id;
use crate::{authentication_layer, config};
use crate::transform_layer::hls_convertor::HlsConvertor;
use crate::utils::log_error::LogError;

pub struct Handler {
    hls_convertor: Arc<HlsConvertor>,
    http_client: Arc<Client>,
}

impl Handler {
    pub fn new(hls_convertor: Arc<HlsConvertor>, client: Arc<Client>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            hls_convertor,
            http_client: client,
        })
    }
}

impl SessionHandler for Handler {
    async fn on_publish(
        &mut self,
        stream_id: u32,
        _app_name: &str,
        stream_key: &str,
    ) -> Result<(), ServerSessionError> {
        if stream_key.is_empty() {
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        // let authed_stream_id: &str = &authenticate_and_get_stream_id(stream_key, &self.http_client).await?;
        let authed_stream_id = stream_key;
        let config = config::get_config();

        if let Err(e) = self.hls_convertor.start_hls_conversion(stream_id, authed_stream_id, &config.server.host) {
            eprintln!("Failed to start HLS conversion: {}", e);
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        let mut header = Vec::new();
        header.extend_from_slice(b"FLV"); // Signature
        header.push(1); // Version
        header.push(0x05); // Flags (audio + video)
        header.extend_from_slice(&9u32.to_be_bytes()); // DataOffset
        header.extend_from_slice(&0u32.to_be_bytes()); // PreviousTagSize0

        let pipelines = self.hls_convertor.get_pipelines();
        let mut pipelines = pipelines.lock().unwrap();
        if let Some(pipeline) = pipelines.get_mut(&stream_id) {
            if let Err(e) = pipeline.stdin.write_all(&header) {
                eprintln!("Failed to write FLV header: {}", e);
            }
        }

        Ok(())
    }

    async fn on_unpublish(&mut self, stream_id: u32) -> Result<(), ServerSessionError> {
        self.hls_convertor.stop_hls_conversion(stream_id);
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

        let pipelines = self.hls_convertor.get_pipelines();
        let mut pipelines = pipelines.lock().unwrap();
        if let Some(pipeline) = pipelines.get_mut(&stream_id) {
            if let Err(e) = pipeline.stdin.write_all(&flv_tag) {
                eprintln!("Failed to write FLV tag: {}", e);
            }
        }

        Ok(())
    }
}