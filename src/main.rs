use scuffle_rtmp::ServerSession;
use reqwest::Client;
use tokio::net::TcpListener;
mod config;
mod handler;
mod m3u8_server;
mod session_handler;
mod authentication_layer;

use handler::Handler;
use m3u8_server::start_m3u8_server_background;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    start_m3u8_server_background();
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    let config = config::get_config();
    let client = Client::new();
    let listener = TcpListener::bind(format!("[::]:{}", config.server.port)).await?;
    println!("RTMP Server listening on [::]:{}", config.server.port);

    while let Ok((stream, addr)) = listener.accept().await {
        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            let handler = match Handler::new(client.clone()) {
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
