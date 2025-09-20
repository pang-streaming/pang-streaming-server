use serde::Deserialize;
use std::fs;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub segment_delay: u32,
    pub port: u16,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let toml_str =
            fs::read_to_string("config.toml").expect("환경변수를 불러오는데 실패했습니다.");
        toml::from_str(&toml_str).expect("환경변수를 파싱하는데 실패했습니다.")
    })
}
