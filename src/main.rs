use scuffle_rtmp::ServerSession;
use tokio::{net::TcpListener, stream};

mod session_handler; // Handler 정의 파일
use session_handler::Handler;

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
