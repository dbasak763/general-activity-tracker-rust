use std::{env, net::SocketAddr};

use url::Url;

use crate::error::AppError;

#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub mongodb_uri: String,
    pub mongodb_database: String,
    pub cors_allowed_origins: Vec<String>,
    pub json_logs: bool,
    pub chat_service_url: Option<String>,
    pub chat_internal_token: String,
    pub chat_max_activities: u64,
    pub chat_timeout_seconds: u64,
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

        let chat_service_url = env::var("CHAT_SERVICE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty());
        if let Some(url) = &chat_service_url {
            let parsed = Url::parse(url).map_err(|error| {
                AppError::configuration(format!("invalid CHAT_SERVICE_URL: {error}"))
            })?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                return Err(AppError::configuration(
                    "CHAT_SERVICE_URL must be an HTTP or HTTPS URL",
                ));
            }
        }
        let chat_internal_token = env::var("CHAT_INTERNAL_TOKEN").unwrap_or_default();
        if chat_service_url.is_some() && chat_internal_token.trim().is_empty() {
            return Err(AppError::configuration(
                "CHAT_INTERNAL_TOKEN is required when CHAT_SERVICE_URL is set",
            ));
        }
        let chat_max_activities = parse_bounded_u64("CHAT_MAX_ACTIVITIES", 2_000, 1, 2_000)?;
        let chat_timeout_seconds = parse_bounded_u64("CHAT_TIMEOUT_SECONDS", 165, 5, 180)?;

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
            chat_service_url,
            chat_internal_token,
            chat_max_activities,
            chat_timeout_seconds,
        })
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, AppError> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|error| AppError::configuration(format!("invalid bind address: {error}")))
    }
}

fn parse_bounded_u64(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, AppError> {
    let value = env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse::<u64>()
        .map_err(|error| AppError::configuration(format!("invalid {name}: {error}")))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(AppError::configuration(format!(
            "{name} must be between {minimum} and {maximum}"
        )));
    }
    Ok(value)
}
