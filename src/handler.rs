use gstreamer::stream;
use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};
use std::sync::{Arc};

use reqwest::Client;
use crate::authentication_layer::auth::authenticate_and_get_stream_id;
use crate::{authentication_layer, config};
use crate::transform_layer::gstreamer::push::push_to_gstreamer;
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

        let authed_stream_id: &str = &authenticate_and_get_stream_id(stream_key, &self.http_client).await?;
        let config = config::get_config();

        if let Err(e) = self.hls_convertor.start_hls_conversion(stream_id, authed_stream_id, &config.server.host) {
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
