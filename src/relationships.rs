//! Explicit, user-confirmed links. Inferred concept links are a derived view.
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{AppState, error::AppError};

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    RelatedTo,
    Addresses,
    Applies,
    InspiredBy,
    PartOf,
    PrerequisiteFor,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationshipInput {
    pub source_id: String,
    pub target_id: String,
    pub kind: RelationshipKind,
    pub reason: String,
}

impl RelationshipInput {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.source_id.trim().is_empty()
            || self.target_id.trim().is_empty()
            || self.source_id.len() > 128
            || self.target_id.len() > 128
            || self.source_id == self.target_id
        {
            return Err(AppError::validation(
                "Choose two different existing activities",
            ));
        }
        if self.reason.trim().is_empty() || self.reason.len() > 1000 {
            return Err(AppError::validation(
                "Relationship reason must contain 1–1000 bytes",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(flatten)]
    pub input: RelationshipInput,
    pub created_at: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationshipFilter {
    pub activity_id: Option<String>,
}

#[utoipa::path(post, path = "/api/relationships", tag = "Relationships", request_body = RelationshipInput,
    responses((status = 201, description = "User-confirmed relationship saved", body = Relationship),
    (status = 404, description = "Activity does not exist"), (status = 422, description = "Invalid relationship")))]
pub async fn save_relationship(
    State(state): State<AppState>,
    Json(input): Json<RelationshipInput>,
) -> Result<(StatusCode, Json<Relationship>), AppError> {
    input.validate()?;
    for id in [&input.source_id, &input.target_id] {
        if state.repository.get(id).await?.is_none() {
            return Err(AppError::NotFound(
                "Relationship activity does not exist".into(),
            ));
        }
    }
    let kind = serde_json::to_value(&input.kind).expect("enum serialization");
    let relationship = Relationship {
        id: format!(
            "{}:{}:{}",
            input.source_id,
            kind.as_str().unwrap(),
            input.target_id
        ),
        input,
        created_at: Utc::now().to_rfc3339(),
    };
    Ok((
        StatusCode::CREATED,
        Json(state.repository.save_relationship(relationship).await?),
    ))
}

#[utoipa::path(get, path = "/api/relationships", tag = "Relationships",
    params(("activityId" = Option<String>, Query, description = "Restrict to one activity")),
    responses((status = 200, description = "Up to 2000 explicit links", body = [Relationship])))]
pub async fn list_relationships(
    State(state): State<AppState>,
    Query(filter): Query<RelationshipFilter>,
) -> Result<Json<Vec<Relationship>>, AppError> {
    Ok(Json(
        state
            .repository
            .list_relationships(filter.activity_id.as_deref())
            .await?,
    ))
}

#[utoipa::path(delete, path = "/api/relationships/{id}", tag = "Relationships",
    params(("id" = String, Path)), responses((status = 204, description = "Relationship removed"), (status = 404, description = "Not found")))]
pub async fn delete_relationship(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if state.repository.delete_relationship(&id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Relationship not found".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_self_links_unknown_kinds_and_empty_reasons() {
        let mut input = RelationshipInput {
            source_id: "one".into(),
            target_id: "two".into(),
            kind: RelationshipKind::Addresses,
            reason: "Practice for this weakness".into(),
        };
        assert!(input.validate().is_ok());
        input.target_id = "one".into();
        assert!(input.validate().is_err());
        input.target_id = "two".into();
        input.reason = " ".into();
        assert!(input.validate().is_err());
        assert!(
            serde_json::from_value::<RelationshipKind>(serde_json::json!("DELETE_ALL")).is_err()
        );
    }
}
