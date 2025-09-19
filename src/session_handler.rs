use reqwest::Client;
use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};
use crate::authentication_layer::auth::authenticate_and_get_stream_id;

pub struct Handler {
    authenticated_stream_id: Option<String>,
    http_client: Client,
}

impl Handler {
    pub fn new(client: Client) -> Self {
        Self {
            authenticated_stream_id: None,
            http_client: client,
        }
    }
}

impl SessionHandler for Handler {
    async fn on_data(
        &mut self,
        stream_id: u32,
        data: SessionData,
    ) -> Result<(), ServerSessionError> {
        match data {
            SessionData::Video { timestamp, data } => {
                println!(
                    "Stream {:?} Video chunk: timestamp={} size={}",
                    self.authenticated_stream_id,
                    timestamp,
                    data.len()
                );
            }
            SessionData::Audio { timestamp, data } => {
                println!(
                    "Stream {} Audio chunk: timestamp={} size={}",
                    stream_id,
                    timestamp,
                    data.len()
                );
            }
            SessionData::Amf0 { timestamp, data } => {
                println!(
                    "Stream {} Metadata: timestamp={} size={}",
                    stream_id,
                    timestamp,
                    data.len()
                );
            }
        }
        Ok(())
    }

    async fn on_publish(
        &mut self,
        stream_id: u32,
        app_name: &str,
        stream_key: &str,
    ) -> Result<(), ServerSessionError> {
        println!("stream_key: {}", stream_key);
        self.authenticated_stream_id = Some(authenticate_and_get_stream_id(stream_key, &self.http_client).await?);
        Ok(())
    }

    // Stream ended
    async fn on_unpublish(&mut self, stream_id: u32) -> Result<(), ServerSessionError> {
        Ok(())
    }
}
