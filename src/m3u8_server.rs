use axum::{
    Router,
    extract::Path,
    http::{StatusCode, header},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub stream_key: String,
    pub title: String,
    pub is_live: bool,
    pub viewer_count: u32,
    pub start_time: u64,
    pub bitrate: u32,
    pub resolution: String,
}

pub struct M3U8Server {}

impl M3U8Server {
    pub fn new() -> Self {
        Self {}
    }
}

async fn get_master_playlist(
    Path(stream_key): Path<String>,
) -> Result<([(String, String); 1], String), StatusCode> {
    let master_playlist = format!(
        "#EXTM3U\n\
         #EXT-X-VERSION:3\n\
         #EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=854x480,CODECS=\"avc1.64001f,mp4a.40.2\"\n\
         {}/playlist.m3u8\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1400000,RESOLUTION=1280x720,CODECS=\"avc1.64001f,mp4a.40.2\"\n\
         {}/playlist.m3u8\n",
        stream_key, stream_key
    );

    Ok((
        [(
            header::CONTENT_TYPE.as_str().to_string(),
            "application/vnd.apple.mpegurl".to_string(),
        )],
        master_playlist,
    ))
}

async fn get_segment_playlist(
    Path(stream_key): Path<String>,
) -> Result<([(String, String); 1], String), StatusCode> {
    let playlist_path = format!("./hls_output/{}/playlist.m3u8", stream_key);

    match fs::read_to_string(&playlist_path).await {
        Ok(content) => Ok((
            [(
                header::CONTENT_TYPE.as_str().to_string(),
                "application/vnd.apple.mpegurl".to_string(),
            )],
            content,
        )),
        Err(_) => {
            let default_playlist = format!(
                "#EXTM3U\n\
                 #EXT-X-VERSION:3\n\
                 #EXT-X-TARGETDURATION:6\n\
                 #EXT-X-MEDIA-SEQUENCE:0\n\
                 #EXT-X-PLAYLIST-TYPE:EVENT\n"
            );
            Ok((
                [(
                    header::CONTENT_TYPE.as_str().to_string(),
                    "application/vnd.apple.mpegurl".to_string(),
                )],
                default_playlist,
            ))
        }
    }
}

async fn get_init_mp4(
    Path(stream_key): Path<String>,
) -> Result<([(String, String); 1], Vec<u8>), StatusCode> {
    let file_path = format!("./hls_output/{}/init.mp4", stream_key);

    match fs::read(&file_path).await {
        Ok(data) => Ok((
            [(
                header::CONTENT_TYPE.as_str().to_string(),
                "video/mp4".to_string(),
            )],
            data,
        )),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_ts_segment(
    Path((stream_key, segment)): Path<(String, String)>,
) -> Result<([(String, String); 1], Vec<u8>), StatusCode> {
    if !segment.ends_with(".ts") {
        return Err(StatusCode::NOT_FOUND);
    }

    let file_path = format!("./hls_output/{}/{}", stream_key, segment);

    match fs::read(&file_path).await {
        Ok(data) => Ok((
            [(
                header::CONTENT_TYPE.as_str().to_string(),
                "video/mp2t".to_string(),
            )],
            data,
        )),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}
pub async fn start_m3u8_server() -> Result<(), Box<dyn std::error::Error>> {
    let server = Arc::new(M3U8Server::new());
    let app = Router::new()
        .route("/hls/{stream_key}/master.m3u8", get(get_master_playlist))
        .route("/hls/{stream_key}/playlist.m3u8", get(get_segment_playlist))
        .route("/hls/{stream_key}/init.mp4", get(get_init_mp4))
        .route("/hls/{stream_key}/{segment}", get(get_ts_segment))
        .layer(CorsLayer::permissive())
        .with_state(server);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn start_m3u8_server_background() {
    tokio::spawn(async {
        if let Err(e) = start_m3u8_server().await {
            eprintln!("Web server error: {}", e);
        }
    });
}
