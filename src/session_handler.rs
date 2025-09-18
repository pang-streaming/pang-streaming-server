use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};

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
        println!("stream_key: {}", stream_key);
        // TODO: add token validate func
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
