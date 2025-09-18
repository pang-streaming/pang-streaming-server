use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct DataResponse {
    username: &'static str,
    #[serde(rename = "startTime")]
    start_time: &'static str
}