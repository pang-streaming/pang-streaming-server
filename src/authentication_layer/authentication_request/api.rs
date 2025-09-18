use reqwest::Client;
use crate::authentication_layer::authentication_request::response::DataResponse;

async fn post_authentication(stream_key: String, client: &Client) -> Result<DataResponse, &'static str> {
    let data = client
        .get("http://localhost:8080/test")
        .header("X-Stream-Key", stream_key)
        .send().await
        .unwrap();
    if data.status().is_success() {
        let response: DataResponse = data.json().await.unwrap();
        Ok(response)
    } else {
        Err("stream key is not allowed")
    }
}