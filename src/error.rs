use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Configuration(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("database operation failed")]
    Database(#[from] mongodb::error::Error),
    #[error("serialization failed: {0}")]
    BsonSerialization(#[from] mongodb::bson::ser::Error),
    #[error("deserialization failed: {0}")]
    BsonDeserialization(#[from] mongodb::bson::de::Error),
    #[error("migration source failed: {0}")]
    MigrationSource(#[from] sqlx::Error),
}

#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub detail: String,
    pub code: &'static str,
}

impl AppError {
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, detail) = match self {
            Self::Configuration(message) => {
                tracing::error!(%message, "configuration error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "configuration_error",
                    message,
                )
            }
            Self::Validation(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                message,
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::Unavailable(message) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                message,
            ),
            Self::Database(error) => {
                tracing::error!(error = %error, "database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "Database operation failed".to_owned(),
                )
            }
            Self::BsonSerialization(error) => {
                tracing::error!(error = %error, "BSON serialization failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "serialization_error",
                    "Serialization failed".to_owned(),
                )
            }
            Self::BsonDeserialization(error) => {
                tracing::error!(error = %error, "BSON deserialization failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    "Deserialization failed".to_owned(),
                )
            }
            Self::MigrationSource(error) => {
                tracing::error!(error = %error, "migration source failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "migration_source_error",
                    "Migration source failed".to_owned(),
                )
            }
        };
        (status, Json(ErrorBody { detail, code })).into_response()
    }
}
