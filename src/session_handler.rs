use reqwest::Client;
use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};
use crate::authentication_layer::auth::authenticate_and_get_stream_id;

pub struct Handler;

impl SessionHandler for Handler {
    async fn on_data(
        &mut self,
        stream_id: u32,
        data: SessionData,
    ) -> Result<(), ServerSessionError> {
        match data {
            SessionData::Video { timestamp, data } => {
                println!(
                    "Stream {} Video chunk: timestamp={} size={}",
                    stream_id,
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

    // When live stream strart
    async fn on_publish(
        &mut self,
        stream_id: u32,
        app_name: &str,
        stream_key: &str,
    ) -> Result<(), ServerSessionError> {
        let client = Client::new();
        println!("stream_key: {}", stream_key);
        let stream_id = authenticate_and_get_stream_id(stream_key, &client).await?;
        if stream_key == "123" {
            println!("stream_id: {}", stream_id);
            println!("app_name: {}", app_name);
            println!("stream_key: {}", stream_key);

            Ok(())
        } else {
            return Err(ServerSessionError::InvalidChunkSize(0));
        }
    }

    // Stream ended
    async fn on_unpublish(&mut self, stream_id: u32) -> Result<(), ServerSessionError> {
        Ok(())
    }
}
