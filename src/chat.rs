use std::{collections::BTreeMap, time::Duration};

use axum::{Json, extract::State};
use chrono::Utc;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    error::{AppError, ErrorBody},
    model::{Activity, ActivityFilter},
    relationships::Relationship,
    routes::AppState,
};

const MAX_RESPONSE_BYTES: usize = 1_048_576;

#[derive(Clone)]
pub struct ChatGateway {
    base_url: Option<Url>,
    token: String,
    max_activities: u64,
    client: Client,
}

impl ChatGateway {
    pub fn new(
        base_url: Option<&str>,
        token: &str,
        max_activities: u64,
        timeout_seconds: u64,
    ) -> Result<Self, AppError> {
        let base_url = base_url
            .map(|value| {
                Url::parse(&format!("{}/", value.trim_end_matches('/'))).map_err(|error| {
                    AppError::configuration(format!("invalid CHAT_SERVICE_URL: {error}"))
                })
            })
            .transpose()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|error| {
                AppError::configuration(format!("could not build chat HTTP client: {error}"))
            })?;
        Ok(Self {
            base_url,
            token: token.to_owned(),
            max_activities,
            client,
        })
    }

    pub fn disabled() -> Self {
        Self::new(None, "", 2_000, 165).expect("default chat client should be valid")
    }

    pub fn configured(&self) -> bool {
        self.base_url.is_some()
    }

    pub fn max_activities(&self) -> u64 {
        self.max_activities
    }

    async fn get_config(&self) -> Result<serde_json::Value, String> {
        let endpoint = self.endpoint("config")?;
        let response = self
            .client
            .get(endpoint)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        if bytes.len() > 131_072 {
            return Err("chat configuration response was too large".to_owned());
        }
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }

    async fn ask(&self, request: &WorkerChatRequest<'_>) -> Result<ChatResponse, String> {
        let endpoint = self.endpoint("chat")?;
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(&self.token)
            .json(request)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("chat response was too large".to_owned());
        }
        let response: ChatResponse =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        response.validate()?;
        Ok(response)
    }

    fn endpoint(&self, path: &str) -> Result<Url, String> {
        self.base_url
            .as_ref()
            .ok_or_else(|| "chat service is not configured".to_owned())?
            .join(path)
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub history: Vec<ChatMessage>,
    pub user_id: Option<String>,
}

impl ChatRequest {
    fn validate(&self) -> Result<(), AppError> {
        validate_text("message", &self.message, 8_000)?;
        if self.history.len() > 10 {
            return Err(AppError::validation("history cannot exceed 10 messages"));
        }
        for message in &self.history {
            if !matches!(message.role.as_str(), "user" | "assistant") {
                return Err(AppError::validation(
                    "history roles must be user or assistant",
                ));
            }
            validate_text("history content", &message.content, 8_000)?;
        }
        if let Some(user_id) = &self.user_id {
            validate_text("userId", user_id, 120)?;
        }
        Ok(())
    }
}

fn validate_text(field: &str, value: &str, maximum: usize) -> Result<(), AppError> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > maximum {
        return Err(AppError::validation(format!(
            "{field} must contain between 1 and {maximum} characters"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatResponse {
    pub reply: String,
    pub provider: String,
    pub model: Option<String>,
    pub route: String,
    #[serde(default)]
    pub operations: Vec<String>,
    #[serde(default)]
    #[schema(value_type = Vec<Object>)]
    pub citations: Vec<serde_json::Value>,
    #[serde(default)]
    #[schema(value_type = Vec<Object>)]
    pub relationships: Vec<serde_json::Value>,
    #[serde(default)]
    #[schema(value_type = Vec<Object>)]
    pub relationship_proposals: Vec<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub visualization: Option<serde_json::Value>,
    pub graph_backend: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[schema(value_type = Object)]
    pub coverage: serde_json::Value,
}

impl ChatResponse {
    fn validate(&self) -> Result<(), String> {
        if self.reply.trim().is_empty() || self.reply.chars().count() > 16_000 {
            return Err("chat worker returned an invalid reply".to_owned());
        }
        if !matches!(
            self.route.as_str(),
            "lookup" | "analysis" | "relationships" | "visualization"
        ) {
            return Err("chat worker returned an invalid route".to_owned());
        }
        if self.operations.len() > 20
            || self.citations.len() > 30
            || self.relationships.len() > 20
            || self.relationship_proposals.len() > 5
            || self.warnings.len() > 20
        {
            return Err("chat worker returned too many result items".to_owned());
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerChatRequest<'a> {
    message: &'a str,
    history: &'a [ChatMessage],
    records: &'a [Activity],
    relationships: &'a [Relationship],
    total_count: u64,
    complete: bool,
    scope: String,
    today: String,
}

#[utoipa::path(
    get,
    path = "/api/dashboard/chat/config",
    tag = "Activity chat",
    responses((status = 200, description = "Configured chat routes and providers"))
)]
pub async fn chat_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    if !state.chat.configured() {
        return Json(unavailable_config("Chat service is not configured"));
    }
    match state.chat.get_config().await {
        Ok(mut config) => {
            if let Some(object) = config.as_object_mut() {
                object.insert("status".to_owned(), serde_json::json!("ready"));
            }
            Json(config)
        }
        Err(error) => {
            tracing::warn!(%error, "chat configuration request failed");
            Json(unavailable_config("Chat service is unavailable"))
        }
    }
}

fn unavailable_config(message: &str) -> serde_json::Value {
    serde_json::json!({
        "status": "unavailable",
        "availableProviders": [],
        "routes": {},
        "graphConfigured": false,
        "message": message
    })
}

#[utoipa::path(
    post,
    path = "/api/dashboard/chat",
    tag = "Activity chat",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Evidence-backed answer across activity types", body = ChatResponse),
        (status = 422, description = "Invalid chat request", body = ErrorBody)
    )
)]
pub async fn chat(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    payload.validate()?;
    let filter = ActivityFilter {
        user_id: payload.user_id.clone(),
        limit: state.chat.max_activities(),
        ..Default::default()
    };
    let (records, total_count) = tokio::try_join!(
        state.repository.list(&filter),
        state.repository.count(&filter)
    )?;
    let record_ids = records
        .iter()
        .map(|record| record.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let relationships = state
        .repository
        .list_relationships(None)
        .await?
        .into_iter()
        .filter(|link| {
            record_ids.contains(link.input.source_id.as_str())
                && record_ids.contains(link.input.target_id.as_str())
        })
        .collect::<Vec<_>>();
    let request = WorkerChatRequest {
        message: payload.message.trim(),
        history: &payload.history,
        records: &records,
        relationships: &relationships,
        total_count,
        complete: records.len() as u64 == total_count,
        scope: format!(
            "{}:{}",
            state.database_name,
            payload.user_id.as_deref().unwrap_or("all-users")
        ),
        today: Utc::now().date_naive().to_string(),
    };
    if state.chat.configured() {
        match state.chat.ask(&request).await {
            Ok(response) => return Ok(Json(response)),
            Err(error) => tracing::warn!(%error, "chat worker request failed"),
        }
    }
    Ok(Json(database_fallback(
        &payload.message,
        &records,
        total_count,
    )))
}

fn database_fallback(message: &str, records: &[Activity], total_count: u64) -> ChatResponse {
    let mut counts = BTreeMap::<String, usize>::new();
    for record in records {
        let name = serde_json::to_value(&record.input.activity_type)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        *counts.entry(name).or_default() += 1;
    }
    let summary = counts
        .iter()
        .map(|(kind, count)| format!("{}: {count}", kind.replace('_', " ")))
        .collect::<Vec<_>>()
        .join(", ");
    let route = classify(message);
    ChatResponse {
        reply: format!(
            "AI models are unavailable. The loaded database snapshot contains {} of {total_count} activities{}.",
            records.len(),
            if summary.is_empty() {
                String::new()
            } else {
                format!(": {summary}")
            }
        ),
        provider: "database".to_owned(),
        model: None,
        route: route.to_owned(),
        operations: vec!["summarize_activities".to_owned()],
        citations: Vec::new(),
        relationships: Vec::new(),
        relationship_proposals: Vec::new(),
        visualization: Some(serde_json::json!({
            "title": "Activity counts in loaded snapshot",
            "labels": counts.keys().cloned().collect::<Vec<_>>(),
            "values": counts.values().copied().collect::<Vec<_>>()
        })),
        graph_backend: "unavailable".to_owned(),
        warnings: vec![
            "The internal AI service is unavailable; this answer contains database counts only."
                .to_owned(),
        ],
        coverage: serde_json::json!({
            "total": total_count,
            "loaded": records.len(),
            "complete": records.len() as u64 == total_count
        }),
    }
}

fn classify(message: &str) -> &'static str {
    let text = message.to_lowercase();
    if [
        "relationship",
        "related",
        "connect",
        "link ",
        "linked",
        "inspired",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        "relationships"
    } else if ["chart", "plot", "visualiz", "visualis"]
        .iter()
        .any(|word| text.contains(word))
    {
        "visualization"
    } else if [
        "compare",
        "weak",
        "recommend",
        "practice next",
        "study next",
        "average",
        "improv",
        "why",
        "trend",
        "progress",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        "analysis"
    } else {
        "lookup"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, extract::State, http::HeaderMap, routing::post};

    #[test]
    fn validates_messages_and_classifies_questions() {
        let request = ChatRequest {
            message: "How are my papers related to experiments?".to_owned(),
            history: vec![ChatMessage {
                role: "assistant".to_owned(),
                content: "Previous answer".to_owned(),
            }],
            user_id: None,
        };
        request.validate().unwrap();
        assert_eq!(classify(&request.message), "relationships");

        let invalid = ChatRequest {
            message: " ".to_owned(),
            history: Vec::new(),
            user_id: None,
        };
        assert!(invalid.validate().is_err());
    }

    #[tokio::test]
    async fn gateway_sends_internal_token_and_validates_worker_response() {
        async fn worker(
            State(expected): State<String>,
            headers: HeaderMap,
            Json(body): Json<serde_json::Value>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some(expected.as_str())
            );
            assert_eq!(body["message"], "Find related work");
            Json(serde_json::json!({
                "reply": "The records share graph search.",
                "provider": "local",
                "model": "qwen3:8b",
                "route": "relationships",
                "operations": ["related_activities"],
                "citations": [],
                "relationships": [],
                "relationshipProposals": [],
                "visualization": null,
                "graphBackend": "derived",
                "warnings": [],
                "coverage": {"total": 0, "loaded": 0, "complete": true}
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/chat", post(worker))
                    .with_state("Bearer private-token".to_owned()),
            )
            .await
        });
        let gateway = ChatGateway::new(
            Some(&format!("http://{address}")),
            "private-token",
            2_000,
            5,
        )
        .unwrap();
        let response = gateway
            .ask(&WorkerChatRequest {
                message: "Find related work",
                history: &[],
                records: &[],
                relationships: &[],
                total_count: 0,
                complete: true,
                scope: "test:all-users".to_owned(),
                today: "2026-09-08".to_owned(),
            })
            .await
            .unwrap();
        assert_eq!(response.provider, "local");
        assert_eq!(response.route, "relationships");
        server.abort();
    }
}
