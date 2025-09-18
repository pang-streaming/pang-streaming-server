use scuffle_rtmp::ServerSession;
use tokio::{net::TcpListener, stream};
mod session_handler;
mod config;

use session_handler::Handler;

#[tokio::main]
async fn main() {
    let config = config::get_config();
    let listener = TcpListener::bind(format!("[::]:{}", config.server.port)).await.unwrap();
    println!("listening on [::]:{}", config.server.port);
    while let Ok((stream, addr)) = listener.accept().await {
        let session = ServerSession::new(stream, Handler);

        tokio::spawn(async move {
            if let Err(err) = session.run().await {
            }
        });
    }
}
