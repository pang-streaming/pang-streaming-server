use reqwest::Client;
use crate::authentication_layer::authentication_request::api::get_authentication;
use crate::authentication_layer::authentication_request::response::StreamUserResponse;

pub async fn authenticate_and_get_stream_id(stream_key: &str, client: & Client) -> Result<String, String> {
    let response: StreamUserResponse = get_authentication(stream_key, client).await
        .expect("Authentication failed");
    let stream_id = format!("{}/{}", response.get_nickname(), response.get_start_time());
    Ok(stream_id)
}