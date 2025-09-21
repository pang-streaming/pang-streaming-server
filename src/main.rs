use std::sync::Arc;
use gstreamer_app::gst;
use scuffle_rtmp::ServerSession;
use reqwest::Client;
use tokio::net::TcpListener;
mod config;
mod handler;
mod m3u8_server;
mod session_handler;
mod authentication_layer;
mod utils;
mod transform_layer;

use handler::Handler;
use m3u8_server::start_m3u8_server_background;
use crate::transform_layer::hls_convertor::HlsConvertor;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    gst::init().expect("Failed to initialize GStreamer");
    start_m3u8_server_background();
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    let config = config::get_config();
    let client = Arc::new(Client::new());
    let hls_convertor = Arc::new(HlsConvertor::new()?);
    let listener = TcpListener::bind(format!("[::]:{}", config.server.port)).await?;
    println!("RTMP Server listening on [::]:{}", config.server.port);

    while let Ok((stream, addr)) = listener.accept().await {
        println!("New connection from: {}", addr);
        let hls_convertor_clone = Arc::clone(&hls_convertor);
        let client_clone = Arc::clone(&client);
        tokio::spawn(async move {
            let handler = match Handler::new(hls_convertor_clone, client_clone) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Failed to create handler for {}: {}", addr, e);
                    return;
                }
            };

            let session = ServerSession::new(stream, handler);

            if let Err(err) = session.run().await {
                eprintln!("Session error from {}: {:?}", addr, err);
            }
        });
    }
    Ok(())
}
