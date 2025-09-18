use scuffle_rtmp::{
    ServerSession,
    session::server::{ServerSessionError, SessionData, SessionHandler},
};
use tokio::{net::TcpListener, stream};

struct Handler;

impl SessionHandler for Handler {
    async fn on_data(
        &mut self,
        stream_id: u32,
        data: SessionData,
    ) -> Result<(), ServerSessionError> {
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

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("[::]:1935").await.unwrap();
    println!("listening on [::]:1935");

    while let Ok((stream, addr)) = listener.accept().await {
        let session = ServerSession::new(stream, Handler);

        tokio::spawn(async move {
            if let Err(err) = session.run().await {
                // Handle the session error
            }
        });
    }
}
