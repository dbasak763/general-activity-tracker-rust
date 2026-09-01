use std::{env, net::SocketAddr};

use url::Url;

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub mongodb_uri: String,
    pub mongodb_database: String,
    pub cors_allowed_origins: Vec<String>,
    pub json_logs: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        dotenvy::dotenv().ok();
        let mongodb_uri = env::var("MONGODB_URI")
            .map_err(|_| AppError::configuration("MONGODB_URI is required"))?;
        Url::parse(&mongodb_uri)
            .map_err(|error| AppError::configuration(format!("invalid MONGODB_URI: {error}")))?;
        let mongodb_database =
            env::var("MONGODB_DATABASE").unwrap_or_else(|_| "activity_tracker".to_owned());
        if mongodb_database.trim().is_empty() {
            return Err(AppError::configuration("MONGODB_DATABASE cannot be empty"));
        }
        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://localhost:5173".to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_owned)
            .collect();

        Ok(Self {
            host: env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_owned()),
            port: env::var("APP_PORT")
                .unwrap_or_else(|_| "8080".to_owned())
                .parse()
                .map_err(|error| AppError::configuration(format!("invalid APP_PORT: {error}")))?,
            mongodb_uri,
            mongodb_database,
            cors_allowed_origins,
            json_logs: env::var("JSON_LOGS")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
                .unwrap_or(false),
        })
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, AppError> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|error| AppError::configuration(format!("invalid bind address: {error}")))
    }
}
