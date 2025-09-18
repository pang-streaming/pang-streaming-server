use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Listening for connections on port 1935");
    let listener = TcpListener::bind("0.0.0.0:1935").await?;

    loop {
        let (stream, connection_info) = listener.accept().await?;

        println!("Connect on {}", connection_info.ip())
    }
}
