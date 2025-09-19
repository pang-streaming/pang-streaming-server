use reqwest::Client;
use crate::authentication_layer::authentication_request::response::StreamUserResponse;

pub async fn get_authentication(stream_key: &str, client: &Client) -> Result<StreamUserResponse, String> {
    let data = client
        .post("http://localhost:8080/stream")
        .header("X-Stream-Key", stream_key)
        .send().await
        .unwrap();
    if data.status().is_success() {
        Ok(data.json().await.unwrap())
    } else {
        Err("stream key is not allowed".to_string())
    }
}