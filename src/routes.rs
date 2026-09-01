use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use mongodb::bson::Document;
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{
    error::AppError,
    model::{
        Activity, ActivityDetails, ActivityFilter, ActivityInput, ActivityStatus, AttemptFilter,
        AttemptSource, InterviewDetails, LegacyAttemptStatus, Priority,
    },
    repository::ActivityRepository,
};

#[derive(Clone)]
pub struct AppState {
    pub repository: Arc<dyn ActivityRepository>,
    pub database_name: String,
}

pub fn app(state: AppState, allowed_origins: &[String]) -> Result<Router, AppError> {
    let origins = allowed_origins
        .iter()
        .map(|value| {
            value.parse::<HeaderValue>().map_err(|error| {
                AppError::configuration(format!("invalid CORS origin {value}: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);

    Ok(Router::new()
        .route("/", get(root))
        .route_service("/dashboard", ServeFile::new("static/dashboard.html"))
        .nest_service("/static", ServeDir::new("static"))
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .route("/database-health", get(database_health))
        .route(
            "/api/activities",
            post(create_activity).get(list_activities),
        )
        .route("/api/activities/count", get(count_activities))
        .route(
            "/api/activities/{id}",
            get(get_activity)
                .put(replace_activity)
                .delete(delete_activity),
        )
        .route("/api/attempts", post(create_attempt).get(list_attempts))
        .route("/api/attempts/count", get(count_attempts))
        .route("/api/attempts/latest", get(latest_attempt))
        .route(
            "/api/attempts/{id}",
            get(get_attempt).put(update_attempt).delete(delete_attempt),
        )
        .route("/api/dashboard/score-history", get(score_history))
        .route("/api/dashboard/score-timeline", get(score_timeline))
        .route("/api/dashboard/topics", get(dashboard_topics))
        .route("/api/dashboard/topic-summaries", get(topic_summaries))
        .route(
            "/api/dashboard/topic-score-progression",
            get(topic_score_progression),
        )
        .route("/api/dashboard/chat/config", get(chat_config))
        .route("/api/dashboard/chat", post(chat))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state))
}

async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({"message": "General Activity Tracker in Rust is running"}))
}
async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "healthy"}))
}

async fn readiness(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    state.repository.ping().await?;
    Ok(Json(
        serde_json::json!({"status": "ready", "database": state.database_name}),
    ))
}

async fn database_health(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.repository.ping().await?;
    Ok(Json(
        serde_json::json!({"status": "healthy", "database": state.database_name}),
    ))
}

async fn create_activity(
    State(state): State<AppState>,
    Json(input): Json<ActivityInput>,
) -> Result<(StatusCode, Json<Activity>), AppError> {
    input.validate_domain()?;
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create(input, None, None).await?),
    ))
}

async fn list_activities(
    State(state): State<AppState>,
    Query(filter): Query<ActivityFilter>,
) -> Result<Json<Vec<Activity>>, AppError> {
    filter.validate()?;
    Ok(Json(state.repository.list(&filter).await?))
}

#[derive(Serialize)]
struct CountResponse {
    count: u64,
}
async fn count_activities(
    State(state): State<AppState>,
    Query(mut filter): Query<ActivityFilter>,
) -> Result<Json<CountResponse>, AppError> {
    filter.limit = 100;
    filter.validate()?;
    Ok(Json(CountResponse {
        count: state.repository.count(&filter).await?,
    }))
}

async fn get_activity(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Activity>, AppError> {
    state
        .repository
        .get(&id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound("Activity not found".to_owned()))
}

async fn replace_activity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ActivityInput>,
) -> Result<Json<Activity>, AppError> {
    input.validate_domain()?;
    state
        .repository
        .replace(&id, input)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound("Activity not found".to_owned()))
}

async fn delete_activity(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if state.repository.delete(&id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Activity not found".to_owned()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttemptCreate {
    pub attempted_date: NaiveDate,
    #[serde(default)]
    pub attempt_source: AttemptSource,
    pub external_attempt_id: Option<String>,
    pub source_url: Option<String>,
    pub challenge_id: Option<String>,
    pub challenge_title: Option<String>,
    pub round_number: Option<u16>,
    pub round_name: Option<String>,
    pub focus_topic: Option<String>,
    pub question_bank_topic_slug: Option<String>,
    pub attempt_number: Option<u32>,
    pub company: Option<String>,
    pub role: Option<String>,
    pub level: Option<String>,
    pub topic: String,
    pub score: Option<f64>,
    #[serde(default)]
    pub status: LegacyAttemptStatus,
    pub notes: Option<String>,
    #[serde(default = "Utc::now")]
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub strengths: Vec<String>,
    pub feedback: Option<String>,
    pub priority_next_drill: Option<String>,
    pub application_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptResponse {
    pub id: i64,
    pub attempted_date: NaiveDate,
    pub attempt_source: AttemptSource,
    pub external_attempt_id: Option<String>,
    pub source_url: Option<String>,
    pub challenge_id: Option<String>,
    pub challenge_title: Option<String>,
    pub round_number: Option<u16>,
    pub round_name: Option<String>,
    pub focus_topic: Option<String>,
    pub question_bank_topic_slug: Option<String>,
    pub attempt_number: Option<u32>,
    pub company: Option<String>,
    pub role: Option<String>,
    pub level: Option<String>,
    pub topic: String,
    pub score: Option<f64>,
    pub status: LegacyAttemptStatus,
    pub notes: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub strengths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_next_drill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
}

fn attempt_to_input(value: AttemptCreate) -> ActivityInput {
    let title = value
        .challenge_title
        .clone()
        .unwrap_or_else(|| value.topic.clone());
    let entity_refs = value
        .application_id
        .clone()
        .map(|id| BTreeMap::from([("application".to_owned(), id)]))
        .unwrap_or_default();
    let details = InterviewDetails {
        topic: value.topic.clone(),
        attempted_date: value.attempted_date,
        attempt_source: value.attempt_source,
        external_attempt_id: value.external_attempt_id,
        challenge_id: value.challenge_id,
        challenge_title: value.challenge_title,
        round_number: value.round_number,
        round_name: value.round_name,
        focus_topic: value.focus_topic,
        question_bank_topic_slug: value.question_bank_topic_slug,
        attempt_number: value.attempt_number,
        company: value.company,
        role: value.role,
        level: value.level,
        strengths: value.strengths,
        priority_next_drill: value.priority_next_drill,
        application_id: value.application_id,
    };
    ActivityInput {
        user_id: "legacy".to_owned(),
        activity_type: crate::model::ActivityType::Interview,
        title,
        description: None,
        category: Some("interview".to_owned()),
        status: match value.status {
            LegacyAttemptStatus::Complete => ActivityStatus::Completed,
            LegacyAttemptStatus::Incomplete => ActivityStatus::Incomplete,
            LegacyAttemptStatus::Invalidated => ActivityStatus::Invalidated,
        },
        priority: Priority::Medium,
        planned_at: None,
        started_at: Some(value.started_at),
        completed_at: value.completed_at,
        duration_minutes: value
            .completed_at
            .and_then(|end| (end - value.started_at).to_std().ok())
            .map(|d| (d.as_secs() / 60) as u32),
        notes: value.notes,
        score: value.score,
        rating: None,
        feedback: value.feedback,
        source_url: value.source_url,
        tags: vec!["interview".to_owned()],
        entity_refs,
        details: ActivityDetails::Interview(details),
        metadata: Document::new(),
    }
}

fn activity_to_attempt(activity: Activity) -> Result<AttemptResponse, AppError> {
    let ActivityDetails::Interview(details) = activity.input.details else {
        return Err(AppError::Validation(
            "activity is not an interview".to_owned(),
        ));
    };
    Ok(AttemptResponse {
        id: activity
            .legacy_attempt_id
            .ok_or_else(|| AppError::Validation("interview has no compatibility ID".to_owned()))?,
        attempted_date: details.attempted_date,
        attempt_source: details.attempt_source,
        external_attempt_id: details.external_attempt_id,
        source_url: activity.input.source_url,
        challenge_id: details.challenge_id,
        challenge_title: details.challenge_title,
        round_number: details.round_number,
        round_name: details.round_name,
        focus_topic: details.focus_topic,
        question_bank_topic_slug: details.question_bank_topic_slug,
        attempt_number: details.attempt_number,
        company: details.company,
        role: details.role,
        level: details.level,
        topic: details.topic,
        score: activity.input.score,
        status: match activity.input.status {
            ActivityStatus::Completed => LegacyAttemptStatus::Complete,
            ActivityStatus::Incomplete => LegacyAttemptStatus::Incomplete,
            ActivityStatus::Invalidated => LegacyAttemptStatus::Invalidated,
            _ => {
                return Err(AppError::Validation(
                    "interview has incompatible status".to_owned(),
                ));
            }
        },
        notes: activity.input.notes,
        started_at: activity.input.started_at.unwrap_or(activity.created_at),
        completed_at: activity.input.completed_at,
        created_at: activity.created_at,
        strengths: details.strengths,
        feedback: activity.input.feedback,
        priority_next_drill: details.priority_next_drill,
        application_id: details.application_id,
    })
}

async fn create_attempt(
    State(state): State<AppState>,
    Json(payload): Json<AttemptCreate>,
) -> Result<(StatusCode, Json<AttemptResponse>), AppError> {
    let id = state.repository.next_attempt_id().await?;
    let input = attempt_to_input(payload);
    input.validate_domain()?;
    let activity = state
        .repository
        .create(input, Some(format!("interview:{id}")), Some(id))
        .await?;
    Ok((StatusCode::CREATED, Json(activity_to_attempt(activity)?)))
}

async fn list_attempts(
    State(state): State<AppState>,
    Query(filter): Query<AttemptFilter>,
) -> Result<Json<Vec<AttemptResponse>>, AppError> {
    filter.validate()?;
    let attempts = state.repository.list_attempts(&filter).await?;
    Ok(Json(
        attempts
            .into_iter()
            .map(activity_to_attempt)
            .collect::<Result<_, _>>()?,
    ))
}

async fn count_attempts(
    State(state): State<AppState>,
    Query(mut filter): Query<AttemptFilter>,
) -> Result<Json<CountResponse>, AppError> {
    filter.limit = 100;
    filter.validate()?;
    Ok(Json(CountResponse {
        count: state.repository.count_attempts(&filter).await?,
    }))
}

async fn latest_attempt(
    State(state): State<AppState>,
    Query(mut filter): Query<AttemptFilter>,
) -> Result<Json<Option<AttemptResponse>>, AppError> {
    filter.limit = 1;
    filter.offset = 0;
    filter.validate()?;
    Ok(Json(
        state
            .repository
            .list_attempts(&filter)
            .await?
            .into_iter()
            .next()
            .map(activity_to_attempt)
            .transpose()?,
    ))
}

async fn get_attempt(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<AttemptResponse>, AppError> {
    let value = state
        .repository
        .get_attempt(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Interview attempt not found".to_owned()))?;
    Ok(Json(activity_to_attempt(value)?))
}

async fn update_attempt(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(patch): Json<serde_json::Value>,
) -> Result<Json<AttemptResponse>, AppError> {
    let existing = state
        .repository
        .get_attempt(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Interview attempt not found".to_owned()))?;
    let current = activity_to_attempt(existing.clone())?;
    let patch = patch
        .as_object()
        .ok_or_else(|| AppError::validation("request body must be a JSON object"))?;
    let mut merged = serde_json::to_value(current)
        .map_err(|error| AppError::validation(format!("could not merge update: {error}")))?;
    let target = merged
        .as_object_mut()
        .ok_or_else(|| AppError::validation("could not prepare update"))?;
    target.remove("id");
    target.remove("createdAt");
    for (key, value) in patch {
        target.insert(key.clone(), value.clone());
    }
    let merged: AttemptCreate =
        serde_json::from_value(merged).map_err(|error| AppError::validation(error.to_string()))?;
    let input = attempt_to_input(merged);
    input.validate_domain()?;
    let updated = state
        .repository
        .replace(&existing.id, input)
        .await?
        .ok_or_else(|| AppError::NotFound("Interview attempt not found".to_owned()))?;
    Ok(Json(activity_to_attempt(updated)?))
}

async fn delete_attempt(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let existing = state
        .repository
        .get_attempt(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Interview attempt not found".to_owned()))?;
    state.repository.delete(&existing.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn all_attempts(state: &AppState) -> Result<Vec<AttemptResponse>, AppError> {
    let filter = AttemptFilter {
        limit: 500,
        ..Default::default()
    };
    state
        .repository
        .list_attempts(&filter)
        .await?
        .into_iter()
        .map(activity_to_attempt)
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScoreHistoryPoint {
    attempted_date: NaiveDate,
    average_score: f64,
}
async fn score_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<ScoreHistoryPoint>>, AppError> {
    let mut grouped: BTreeMap<NaiveDate, Vec<f64>> = BTreeMap::new();
    for item in all_attempts(&state).await? {
        if let Some(score) = item.score {
            grouped.entry(item.attempted_date).or_default().push(score);
        }
    }
    Ok(Json(
        grouped
            .into_iter()
            .map(|(attempted_date, scores)| ScoreHistoryPoint {
                attempted_date,
                average_score: scores.iter().sum::<f64>() / scores.len() as f64,
            })
            .collect(),
    ))
}

async fn score_timeline(
    State(state): State<AppState>,
) -> Result<Json<Vec<AttemptResponse>>, AppError> {
    let mut values: Vec<_> = all_attempts(&state)
        .await?
        .into_iter()
        .filter(|item| item.score.is_some())
        .collect();
    values.sort_by_key(|item| (item.started_at, item.id));
    Ok(Json(values))
}

fn canonical_topic(topic: &str, focus: Option<&str>) -> String {
    let haystack = format!("{} {}", topic, focus.unwrap_or_default()).to_lowercase();
    if ["graph", "algorithm", "data structure", "complexity", "scc"]
        .iter()
        .any(|v| haystack.contains(v))
    {
        "Algorithms & Data Structures".to_owned()
    } else if ["deep learning", "backprop", "neural"]
        .iter()
        .any(|v| haystack.contains(v))
    {
        "Deep Learning".to_owned()
    } else if ["behavioral", "communication", "motivation", "fit"]
        .iter()
        .any(|v| haystack.contains(v))
    {
        "Behavioral & Communication".to_owned()
    } else {
        topic.to_owned()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TopicCount {
    topic: String,
    attempt_count: usize,
    focus_topic_count: usize,
}
async fn dashboard_topics(
    State(state): State<AppState>,
) -> Result<Json<Vec<TopicCount>>, AppError> {
    let mut grouped: BTreeMap<String, (usize, std::collections::BTreeSet<String>)> =
        BTreeMap::new();
    for item in all_attempts(&state)
        .await?
        .into_iter()
        .filter(|a| a.status == LegacyAttemptStatus::Complete && a.score.is_some())
    {
        let topic = canonical_topic(&item.topic, item.focus_topic.as_deref());
        let bucket = grouped.entry(topic).or_default();
        bucket.0 += 1;
        if let Some(focus) = item.focus_topic {
            bucket.1.insert(focus);
        }
    }
    Ok(Json(
        grouped
            .into_iter()
            .map(|(topic, (attempt_count, focus))| TopicCount {
                topic,
                attempt_count,
                focus_topic_count: focus.len(),
            })
            .collect(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TopicSummary {
    topic: String,
    attempt_count: usize,
    average_score: f64,
    lowest_score: f64,
    highest_score: f64,
    first_score: f64,
    latest_score: f64,
    score_change: f64,
}
async fn topic_summaries(
    State(state): State<AppState>,
) -> Result<Json<Vec<TopicSummary>>, AppError> {
    let mut attempts = all_attempts(&state).await?;
    attempts.sort_by_key(|a| (a.attempted_date, a.started_at, a.id));
    let mut grouped: HashMap<String, Vec<f64>> = HashMap::new();
    for item in attempts
        .into_iter()
        .filter(|a| a.status == LegacyAttemptStatus::Complete)
    {
        if let Some(score) = item.score {
            grouped
                .entry(canonical_topic(&item.topic, item.focus_topic.as_deref()))
                .or_default()
                .push(score);
        }
    }
    let mut result: Vec<_> = grouped
        .into_iter()
        .map(|(topic, scores)| {
            let first = scores[0];
            let latest = scores[scores.len() - 1];
            TopicSummary {
                topic,
                attempt_count: scores.len(),
                average_score: scores.iter().sum::<f64>() / scores.len() as f64,
                lowest_score: scores.iter().copied().fold(f64::INFINITY, f64::min),
                highest_score: scores.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                first_score: first,
                latest_score: latest,
                score_change: latest - first,
            }
        })
        .collect();
    result.sort_by(|a, b| a.average_score.total_cmp(&b.average_score));
    Ok(Json(result))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TopicProgression {
    topic: String,
    focus_topics: Vec<String>,
    points: Vec<AttemptResponse>,
}
async fn topic_score_progression(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<TopicProgression>, AppError> {
    let topic = params
        .get("topic")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation("topic is required"))?
        .to_owned();
    let mut points: Vec<_> = all_attempts(&state)
        .await?
        .into_iter()
        .filter(|a| {
            a.status == LegacyAttemptStatus::Complete
                && a.score.is_some()
                && canonical_topic(&a.topic, a.focus_topic.as_deref()) == topic
        })
        .collect();
    points.sort_by_key(|a| (a.attempted_date, a.started_at, a.id));
    if points.is_empty() {
        return Err(AppError::NotFound(format!(
            "No completed scores found for topic: {topic}"
        )));
    }
    let mut focus_topics: Vec<_> = points
        .iter()
        .filter_map(|p| p.focus_topic.clone())
        .collect();
    focus_topics.sort();
    focus_topics.dedup();
    Ok(Json(TopicProgression {
        topic,
        focus_topics,
        points,
    }))
}

async fn chat_config() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"availableProviders": [], "routes": {"lookup":{"provider":"fallback"},"analysis":{"provider":"fallback"},"visualization":{"provider":"fallback"}}}),
    )
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    #[serde(default)]
    topic: Option<String>,
}
async fn chat(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if payload.message.trim().is_empty() {
        return Err(AppError::validation("message cannot be empty"));
    }
    let count = state
        .repository
        .count_attempts(&AttemptFilter {
            topic: payload.topic.clone(),
            limit: 100,
            ..Default::default()
        })
        .await?;
    let scope = payload
        .topic
        .map(|t| format!(" for {t}"))
        .unwrap_or_default();
    Ok(Json(
        serde_json::json!({"reply": format!("I found {count} interview attempts{scope}."), "provider":"database", "model":null, "route":"lookup", "operations":["count_attempts"], "visualization":null}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use std::sync::Mutex;
    use tower::ServiceExt;

    #[derive(Default)]
    struct MemoryRepository {
        activity: Mutex<Option<Activity>>,
    }

    #[async_trait]
    impl ActivityRepository for MemoryRepository {
        async fn ping(&self) -> Result<(), AppError> {
            Ok(())
        }
        async fn ensure_indexes(&self) -> Result<(), AppError> {
            Ok(())
        }
        async fn create(
            &self,
            input: ActivityInput,
            id: Option<String>,
            legacy_id: Option<i64>,
        ) -> Result<Activity, AppError> {
            let now = Utc::now();
            let activity = Activity {
                id: id.unwrap_or_else(|| "test".to_owned()),
                input,
                legacy_attempt_id: legacy_id,
                created_at: now,
                updated_at: now,
            };
            *self.activity.lock().unwrap() = Some(activity.clone());
            Ok(activity)
        }
        async fn upsert(&self, activity: Activity) -> Result<bool, AppError> {
            *self.activity.lock().unwrap() = Some(activity);
            Ok(true)
        }
        async fn get(&self, id: &str) -> Result<Option<Activity>, AppError> {
            Ok(self.activity.lock().unwrap().clone().filter(|a| a.id == id))
        }
        async fn list(&self, _: &ActivityFilter) -> Result<Vec<Activity>, AppError> {
            Ok(self.activity.lock().unwrap().clone().into_iter().collect())
        }
        async fn count(&self, _: &ActivityFilter) -> Result<u64, AppError> {
            Ok(self.activity.lock().unwrap().is_some() as u64)
        }
        async fn replace(
            &self,
            id: &str,
            input: ActivityInput,
        ) -> Result<Option<Activity>, AppError> {
            let mut stored = self.activity.lock().unwrap();
            let Some(existing) = stored.as_ref() else {
                return Ok(None);
            };
            if existing.id != id {
                return Ok(None);
            }
            let activity = Activity {
                id: id.to_owned(),
                input,
                legacy_attempt_id: existing.legacy_attempt_id,
                created_at: existing.created_at,
                updated_at: Utc::now(),
            };
            *stored = Some(activity.clone());
            Ok(Some(activity))
        }
        async fn delete(&self, id: &str) -> Result<bool, AppError> {
            let mut stored = self.activity.lock().unwrap();
            if stored.as_ref().is_some_and(|a| a.id == id) {
                *stored = None;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        async fn list_attempts(&self, _: &AttemptFilter) -> Result<Vec<Activity>, AppError> {
            Ok(self.activity.lock().unwrap().clone().into_iter().collect())
        }
        async fn count_attempts(&self, _: &AttemptFilter) -> Result<u64, AppError> {
            Ok(self.activity.lock().unwrap().is_some() as u64)
        }
        async fn get_attempt(&self, legacy_id: i64) -> Result<Option<Activity>, AppError> {
            Ok(self
                .activity
                .lock()
                .unwrap()
                .clone()
                .filter(|a| a.legacy_attempt_id == Some(legacy_id)))
        }
        async fn next_attempt_id(&self) -> Result<i64, AppError> {
            Ok(7)
        }
        async fn seed_attempt_counter(&self, _: i64) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[test]
    fn canonicalizes_source_topics() {
        assert_eq!(
            canonical_topic("Hard graph problem", Some("SCCs")),
            "Algorithms & Data Structures"
        );
        assert_eq!(
            canonical_topic("Deep Learning", Some("Backpropagation")),
            "Deep Learning"
        );
    }

    #[tokio::test]
    async fn creates_legacy_attempt_with_compatible_response() {
        let router = app(
            AppState {
                repository: Arc::new(MemoryRepository::default()),
                database_name: "test".to_owned(),
            },
            &[],
        )
        .unwrap();
        let payload = serde_json::json!({
            "attemptedDate":"2026-09-01", "attemptSource":"manual", "topic":"System Design",
            "score":88.5, "status":"complete", "startedAt":"2026-09-01T12:00:00Z"
        });
        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/attempts")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(body["id"], 7);
        assert_eq!(body["status"], "complete");
        assert_eq!(body["topic"], "System Design");
    }

    #[tokio::test]
    async fn readiness_checks_repository() {
        let router = app(
            AppState {
                repository: Arc::new(MemoryRepository::default()),
                database_name: "test".to_owned(),
            },
            &[],
        )
        .unwrap();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn update_can_explicitly_clear_nullable_fields() {
        let repository = Arc::new(MemoryRepository::default());
        let router = app(
            AppState {
                repository,
                database_name: "test".to_owned(),
            },
            &[],
        )
        .unwrap();
        let create = serde_json::json!({"attemptedDate":"2026-09-01","topic":"System Design","company":"Example Co","score":80,"status":"complete","startedAt":"2026-09-01T12:00:00Z"});
        let created = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/attempts")
                    .header("content-type", "application/json")
                    .body(Body::from(create.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let updated = router
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/attempts/7")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"company":null}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(updated.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(updated.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert!(body["company"].is_null());
    }
}
