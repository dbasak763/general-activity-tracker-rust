use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};

use crate::{
    error::AppError,
    model::{
        Activity, ActivityDetails, ActivityInput, ActivityStatus, AttemptSource, InterviewDetails,
        Priority, metadata_with_legacy_id,
    },
    repository::ActivityRepository,
};

#[derive(Clone, Debug, FromRow)]
pub struct PostgresAttempt {
    pub id: i64,
    pub attempted_date: NaiveDate,
    pub attempt_source: String,
    pub external_attempt_id: Option<String>,
    pub source_url: Option<String>,
    pub challenge_id: Option<String>,
    pub challenge_title: Option<String>,
    pub round_number: Option<i16>,
    pub round_name: Option<String>,
    pub focus_topic: Option<String>,
    pub question_bank_topic_slug: Option<String>,
    pub attempt_number: Option<i32>,
    pub company: Option<String>,
    pub role: Option<String>,
    pub level: Option<String>,
    pub topic: String,
    pub score: Option<f64>,
    pub status: String,
    pub notes: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub source_count: usize,
    pub mapped_count: usize,
    pub inserted_count: usize,
    pub updated_count: usize,
    pub verified_samples: usize,
    pub dry_run: bool,
    pub max_legacy_id: Option<i64>,
}

pub fn map_postgres_attempt(row: PostgresAttempt) -> Result<Activity, AppError> {
    let attempt_source = match row.attempt_source.as_str() {
        "manual" => AttemptSource::Manual,
        "casual" => AttemptSource::Casual,
        "challenge" => AttemptSource::Challenge,
        "question_bank" => AttemptSource::QuestionBank,
        value => {
            return Err(AppError::validation(format!(
                "legacy row {} has unknown attempt_source {value}",
                row.id
            )));
        }
    };
    let status = match row.status.as_str() {
        "complete" => ActivityStatus::Completed,
        "incomplete" => ActivityStatus::Incomplete,
        "invalidated" => ActivityStatus::Invalidated,
        value => {
            return Err(AppError::validation(format!(
                "legacy row {} has unknown status {value}",
                row.id
            )));
        }
    };
    let details = InterviewDetails {
        topic: row.topic.clone(),
        attempted_date: row.attempted_date,
        attempt_source,
        external_attempt_id: row.external_attempt_id,
        challenge_id: row.challenge_id,
        challenge_title: row.challenge_title.clone(),
        round_number: row.round_number.map(|v| v as u16),
        round_name: row.round_name,
        focus_topic: row.focus_topic,
        question_bank_topic_slug: row.question_bank_topic_slug,
        attempt_number: row.attempt_number.map(|v| v as u32),
        company: row.company,
        role: row.role,
        level: row.level,
        strengths: Vec::new(),
        priority_next_drill: None,
        application_id: None,
    };
    let input = ActivityInput {
        user_id: "legacy".to_owned(),
        activity_type: crate::model::ActivityType::Interview,
        title: row.challenge_title.unwrap_or_else(|| row.topic.clone()),
        description: None,
        category: Some("interview".to_owned()),
        status,
        priority: Priority::Medium,
        planned_at: None,
        started_at: Some(row.started_at),
        completed_at: row.completed_at,
        duration_minutes: row
            .completed_at
            .and_then(|end| (end - row.started_at).to_std().ok())
            .map(|d| (d.as_secs() / 60) as u32),
        notes: row.notes,
        score: row.score,
        rating: None,
        feedback: None,
        source_url: row.source_url,
        tags: vec!["interview".to_owned(), "postgresql-migration".to_owned()],
        entity_refs: Default::default(),
        details: ActivityDetails::Interview(details),
        metadata: metadata_with_legacy_id(row.id),
    };
    input.validate_domain()?;
    Ok(Activity {
        id: format!("legacy:interview_attempts:{}", row.id),
        input,
        legacy_attempt_id: Some(row.id),
        created_at: row.created_at,
        updated_at: row.created_at,
    })
}

pub async fn load_postgres_attempts(pool: &PgPool) -> Result<Vec<PostgresAttempt>, AppError> {
    Ok(sqlx::query_as::<_, PostgresAttempt>(
        r#"
        SELECT id, attempted_date, attempt_source, external_attempt_id, source_url,
               challenge_id, challenge_title, round_number, round_name, focus_topic,
               question_bank_topic_slug, attempt_number, company, role, level, topic,
               score::float8 AS score, status, notes, started_at, completed_at, created_at
        FROM interview_attempts ORDER BY id
    "#,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn migrate(
    rows: Vec<PostgresAttempt>,
    repository: Arc<dyn ActivityRepository>,
    dry_run: bool,
) -> Result<MigrationReport, AppError> {
    let mut report = MigrationReport {
        source_count: rows.len(),
        dry_run,
        max_legacy_id: rows.iter().map(|row| row.id).max(),
        ..Default::default()
    };
    let activities = rows
        .into_iter()
        .map(map_postgres_attempt)
        .collect::<Result<Vec<_>, _>>()?;
    report.mapped_count = activities.len();
    if dry_run {
        return Ok(report);
    }

    for activity in &activities {
        if repository.upsert(activity.clone()).await? {
            report.inserted_count += 1;
        } else {
            report.updated_count += 1;
        }
    }
    if let Some(maximum) = report.max_legacy_id {
        repository.seed_attempt_counter(maximum).await?;
    }
    for index in representative_indexes(activities.len()) {
        let expected = &activities[index];
        let actual = repository.get(&expected.id).await?.ok_or_else(|| {
            AppError::NotFound(format!("migrated sample {} was not found", expected.id))
        })?;
        if actual.legacy_attempt_id != expected.legacy_attempt_id
            || actual.input.title != expected.input.title
        {
            return Err(AppError::Conflict(format!(
                "migrated sample {} did not match source mapping",
                expected.id
            )));
        }
        report.verified_samples += 1;
    }
    Ok(report)
}

fn representative_indexes(length: usize) -> Vec<usize> {
    if length == 0 {
        return Vec::new();
    }
    let mut indexes = vec![0, length / 2, length - 1];
    indexes.sort_unstable();
    indexes.dedup();
    indexes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_legacy_interview_field_and_stable_id() {
        let started = Utc::now();
        let row = PostgresAttempt {
            id: 42,
            attempted_date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            attempt_source: "challenge".to_owned(),
            external_attempt_id: Some("ext-42".to_owned()),
            source_url: Some("https://example.com/attempt/42".to_owned()),
            challenge_id: Some("challenge-42".to_owned()),
            challenge_title: Some("Senior backend".to_owned()),
            round_number: Some(2),
            round_name: Some("Coding".to_owned()),
            focus_topic: Some("Graphs".to_owned()),
            question_bank_topic_slug: Some("graphs".to_owned()),
            attempt_number: Some(1),
            company: Some("Example".to_owned()),
            role: Some("Engineer".to_owned()),
            level: Some("Senior".to_owned()),
            topic: "Algorithms".to_owned(),
            score: Some(88.5),
            status: "complete".to_owned(),
            notes: Some("Strong solution".to_owned()),
            started_at: started,
            completed_at: Some(started + chrono::Duration::minutes(45)),
            created_at: started,
        };
        let activity = map_postgres_attempt(row).unwrap();
        assert_eq!(activity.id, "legacy:interview_attempts:42");
        assert_eq!(activity.legacy_attempt_id, Some(42));
        assert_eq!(activity.input.score, Some(88.5));
        let ActivityDetails::Interview(details) = activity.input.details else {
            panic!("wrong type")
        };
        assert_eq!(details.round_number, Some(2));
        assert_eq!(details.question_bank_topic_slug.as_deref(), Some("graphs"));
    }
}
