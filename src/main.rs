use reqwest::Client;
use scuffle_rtmp::ServerSession;
use tokio::{net::TcpListener, stream};
use crate::session_handler::Handler;

mod session_handler;
mod config;
mod authentication_layer;

#[tokio::main]
async fn main() {
    let config = config::get_config();
    let listener = TcpListener::bind(format!("[::]:{}", config.server.port)).await.unwrap();
    let client = Client::new();
    println!("listening on [::]:{}", config.server.port);
    while let Ok((stream, addr)) = listener.accept().await {
        let session = ServerSession::new(stream, Handler::new(client.clone()));

        tokio::spawn(async move {
            if let Err(err) = session.run().await {}
        });
    }
}