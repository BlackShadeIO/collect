use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub api_key: String,
    pub api_host: String,
    pub api_port: u16,
    pub data_dir: PathBuf,
    pub log_level: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let api_key = std::env::var("API_KEY").unwrap_or_else(|_| "changeme".to_string());
        let api_host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?;
        let data_dir = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()));
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        Ok(Self {
            api_key,
            api_host,
            api_port,
            data_dir,
            log_level,
        })
    }

    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.api_host, self.api_port)
    }
}
