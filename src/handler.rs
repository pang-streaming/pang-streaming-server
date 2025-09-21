use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};
use std::sync::{Arc};

use gstreamer::prelude::*;
use reqwest::Client;
use crate::transform_layer::gstreamer::push::push_to_gstreamer;
use crate::transform_layer::hls_convertor::HlsConvertor;
use crate::utils::log_error::LogError;

pub struct Handler {
    hls_convertor: Arc<HlsConvertor>,
    http_client: Arc<Client>,
    authenticated_stream_id: Option<String>,
}

impl Handler {
    pub fn new(hls_convertor: Arc<HlsConvertor>, client: Arc<Client>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            hls_convertor,
            http_client: client,
            authenticated_stream_id: None
        })
    }
}

impl SessionHandler for Handler {
    async fn on_publish(
        &mut self,
        stream_id: u32,
        app_name: &str,
        stream_key: &str,
    ) -> Result<(), ServerSessionError> {
        if stream_key.is_empty() {
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        if let Err(e) = self.hls_convertor.start_hls_conversion(stream_id, stream_key) {
            eprintln!("Failed to start HLS conversion: {}", e);
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        let flv_header = self.hls_convertor.create_flv_header();
        let _ = push_to_gstreamer(self.hls_convertor.get_pipelines(), stream_id, flv_header, 0);
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

        let flv_tag = self.hls_convertor.create_flv_tag(tag_type, timestamp, &payload);
        push_to_gstreamer(self.hls_convertor.get_pipelines(), stream_id, flv_tag, timestamp).log_error("push_failed");
        Ok(())
    }
}
