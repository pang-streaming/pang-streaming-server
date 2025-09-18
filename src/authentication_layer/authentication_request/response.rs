use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct BaseStreamUserResponse {
    status: String,
    message: String,
    data: StreamUserResponse,
    timestamp: String
}

#[derive(Deserialize, Debug)]
pub struct StreamUserResponse {
    nickname: String,
    #[serde(rename = "createdAt")]
    created_at: String
}

impl StreamUserResponse {
    pub fn get_nickname(&self) -> String {
        self.nickname.clone()
    }

    pub fn get_start_time(&self) -> String {
        self.created_at.clone()
    }
}