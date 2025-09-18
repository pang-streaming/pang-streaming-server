use scuffle_rtmp::{
    ServerSession,
    session::server::{ServerSessionError, SessionData, SessionHandler},
};
use tokio::net::TcpListener;

struct Handler;

impl SessionHandler for Handler {
    async fn on_data(
        &mut self,
        stream_id: u32,
        data: SessionData,
    ) -> Result<(), ServerSessionError> {
        // Handle incoming video/audio/meta data
        Ok(())
    }

    async fn on_publish(
        &mut self,
        stream_id: u32,
        app_name: &str,
        stream_name: &str,
    ) -> Result<(), ServerSessionError> {
        // Handle the publish event
        Ok(())
    }

    async fn on_unpublish(&mut self, stream_id: u32) -> Result<(), ServerSessionError> {
        // Handle the unpublish event
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("[::]:1935").await.unwrap();
    // listening on [::]:1935

    while let Ok((stream, addr)) = listener.accept().await {
        let session = ServerSession::new(stream, Handler);

        tokio::spawn(async move {
            if let Err(err) = session.run().await {
                // Handle the session error
            }
        });
    }
}
