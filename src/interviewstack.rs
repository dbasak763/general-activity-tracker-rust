use std::{collections::HashSet, io::BufRead, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use mongodb::bson::{Bson, Document};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    model::{
        Activity, ActivityDetails, ActivityInput, ActivityStatus, AttemptSource, InterviewDetails,
        Priority,
    },
    repository::ActivityRepository,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewStackRow {
    pub source_key: String,
    pub date: NaiveDate,
    pub company: String,
    pub role: String,
    pub level: String,
    pub topic: String,
    pub score: Option<f64>,
    #[serde(rename = "type")]
    pub attempt_type: InterviewStackAttemptType,
    pub status: InterviewStackStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterviewStackAttemptType {
    Casual,
    Challenge,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterviewStackStatus {
    Complete,
    Incomplete,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub line: usize,
    pub source_key: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewStackImportReport {
    pub total_lines: usize,
    pub valid_rows: usize,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub verified_count: usize,
    pub verified_samples: usize,
    pub dry_run: bool,
    pub errors: Vec<ImportFailure>,
}

impl InterviewStackRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_key.is_empty() || self.source_key.len() > 100 {
            return Err("sourceKey must contain 1-100 characters".to_owned());
        }
        if !self
            .source_key
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
        {
            return Err("sourceKey may contain only ASCII letters, digits, '-' and '_'".to_owned());
        }
        for (name, value, maximum) in [
            ("company", self.company.as_str(), 150),
            ("role", self.role.as_str(), 150),
            ("level", self.level.as_str(), 100),
            ("topic", self.topic.as_str(), 200),
        ] {
            if value.trim().is_empty() || value.len() > maximum {
                return Err(format!("{name} must contain 1-{maximum} characters"));
            }
        }
        if self
            .score
            .is_some_and(|score| !score.is_finite() || !(0.0..=100.0).contains(&score))
        {
            return Err("score must be between 0 and 100".to_owned());
        }
        match (self.status, self.score) {
            (InterviewStackStatus::Complete, None) => {
                Err("completed records require a score".to_owned())
            }
            (InterviewStackStatus::Incomplete, Some(_)) => {
                Err("incomplete records must have a null score".to_owned())
            }
            _ => Ok(()),
        }
    }

    pub fn stable_id(&self) -> String {
        format!("interviewstack:{}", self.source_key)
    }

    fn source_document(&self) -> Result<Document, AppError> {
        Ok(mongodb::bson::to_document(self)?)
    }
}

pub fn parse_ndjson<R: BufRead>(
    reader: R,
) -> (Vec<(usize, InterviewStackRow)>, Vec<ImportFailure>) {
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut source_keys = HashSet::new();
    for (offset, line) in reader.lines().enumerate() {
        let line_number = offset + 1;
        let line = match line {
            Ok(value) => value,
            Err(error) => {
                failures.push(ImportFailure {
                    line: line_number,
                    source_key: None,
                    message: error.to_string(),
                });
                continue;
            }
        };
        if line.trim().is_empty() {
            failures.push(ImportFailure {
                line: line_number,
                source_key: None,
                message: "blank lines are not valid NDJSON records".to_owned(),
            });
            continue;
        }
        let row: InterviewStackRow = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                failures.push(ImportFailure {
                    line: line_number,
                    source_key: None,
                    message: error.to_string(),
                });
                continue;
            }
        };
        if let Err(message) = row.validate() {
            failures.push(ImportFailure {
                line: line_number,
                source_key: Some(row.source_key),
                message,
            });
            continue;
        }
        if !source_keys.insert(row.source_key.clone()) {
            failures.push(ImportFailure {
                line: line_number,
                source_key: Some(row.source_key),
                message: "duplicate sourceKey in import file".to_owned(),
            });
            continue;
        }
        rows.push((line_number, row));
    }
    (rows, failures)
}

fn map_row(
    row: &InterviewStackRow,
    legacy_attempt_id: i64,
    created_at: DateTime<Utc>,
) -> Result<Activity, AppError> {
    let source_record = row.source_document()?;
    let attempt_source = match row.attempt_type {
        InterviewStackAttemptType::Casual => AttemptSource::Casual,
        InterviewStackAttemptType::Challenge => AttemptSource::Challenge,
    };
    let status = match row.status {
        InterviewStackStatus::Complete => ActivityStatus::Completed,
        InterviewStackStatus::Incomplete => ActivityStatus::Incomplete,
    };
    let mut metadata = Document::new();
    metadata.insert("importSource", "interviewstack.history");
    metadata.insert("sourceKey", row.source_key.clone());
    metadata.insert("sourceRecord", Bson::Document(source_record));
    let input = ActivityInput {
        user_id: "interviewstack".to_owned(),
        activity_type: crate::model::ActivityType::Interview,
        title: row.topic.clone(),
        description: None,
        category: Some("interview".to_owned()),
        status,
        priority: Priority::Medium,
        planned_at: None,
        started_at: None,
        completed_at: None,
        duration_minutes: None,
        notes: None,
        score: row.score,
        rating: None,
        feedback: None,
        source_url: None,
        tags: vec!["interview".to_owned(), "interviewstack".to_owned()],
        entity_refs: Default::default(),
        details: ActivityDetails::Interview(InterviewDetails {
            topic: row.topic.clone(),
            attempted_date: row.date,
            attempt_source,
            external_attempt_id: Some(row.source_key.clone()),
            challenge_id: None,
            challenge_title: None,
            round_number: None,
            round_name: None,
            focus_topic: None,
            question_bank_topic_slug: None,
            attempt_number: None,
            company: Some(row.company.clone()),
            role: Some(row.role.clone()),
            level: Some(row.level.clone()),
            strengths: Vec::new(),
            priority_next_drill: None,
            application_id: None,
        }),
        metadata,
    };
    input.validate_domain()?;
    Ok(Activity {
        id: row.stable_id(),
        input,
        legacy_attempt_id: Some(legacy_attempt_id),
        created_at,
        updated_at: Utc::now(),
    })
}

pub fn dry_run_report(
    rows: &[(usize, InterviewStackRow)],
    failures: Vec<ImportFailure>,
    total_lines: usize,
) -> InterviewStackImportReport {
    let mut report = InterviewStackImportReport {
        total_lines,
        valid_rows: rows.len(),
        failed: failures.len(),
        dry_run: true,
        errors: failures,
        ..Default::default()
    };
    for (line, row) in rows {
        if let Err(error) = map_row(row, *line as i64, Utc::now()) {
            report.valid_rows -= 1;
            report.failed += 1;
            report.errors.push(ImportFailure {
                line: *line,
                source_key: Some(row.source_key.clone()),
                message: error.to_string(),
            });
        }
    }
    report
}

pub async fn import_rows(
    rows: &[(usize, InterviewStackRow)],
    failures: Vec<ImportFailure>,
    total_lines: usize,
    repository: Arc<dyn ActivityRepository>,
) -> Result<InterviewStackImportReport, AppError> {
    let mut report = InterviewStackImportReport {
        total_lines,
        valid_rows: rows.len(),
        failed: failures.len(),
        errors: failures,
        ..Default::default()
    };

    for (line, row) in rows {
        let existing = repository.get(&row.stable_id()).await?;
        if existing.as_ref().is_some_and(|activity| {
            activity.input.metadata.get_document("sourceRecord").ok()
                == row.source_document().ok().as_ref()
        }) {
            report.skipped += 1;
            continue;
        }
        let (legacy_id, created_at, is_new) = if let Some(activity) = existing {
            let legacy_id = activity.legacy_attempt_id.ok_or_else(|| {
                AppError::Conflict(format!(
                    "existing {} has no compatibility ID",
                    row.stable_id()
                ))
            })?;
            (legacy_id, activity.created_at, false)
        } else {
            (repository.next_attempt_id().await?, Utc::now(), true)
        };
        match map_row(row, legacy_id, created_at) {
            Ok(activity) => {
                repository.upsert(activity).await?;
                if is_new {
                    report.inserted += 1;
                } else {
                    report.updated += 1;
                }
            }
            Err(error) => {
                report.valid_rows -= 1;
                report.failed += 1;
                report.errors.push(ImportFailure {
                    line: *line,
                    source_key: Some(row.source_key.clone()),
                    message: error.to_string(),
                });
            }
        }
    }

    for (_, row) in rows {
        let activity = repository.get(&row.stable_id()).await?.ok_or_else(|| {
            AppError::NotFound(format!(
                "import verification could not find {}",
                row.stable_id()
            ))
        })?;
        let actual = activity
            .input
            .metadata
            .get_document("sourceRecord")
            .map_err(|_| {
                AppError::Conflict(format!("sourceRecord is missing for {}", row.source_key))
            })?;
        if actual != &row.source_document()? {
            return Err(AppError::Conflict(format!(
                "source verification failed for {}",
                row.source_key
            )));
        }
        report.verified_count += 1;
    }
    for index in representative_indexes(rows.len()) {
        let row = &rows[index].1;
        let activity = repository
            .get(&row.stable_id())
            .await?
            .ok_or_else(|| AppError::NotFound(row.stable_id()))?;
        let actual = activity
            .input
            .metadata
            .get_document("sourceRecord")
            .map_err(|_| {
                AppError::Conflict(format!("sourceRecord is missing for {}", row.source_key))
            })?;
        if actual != &row.source_document()? {
            return Err(AppError::Conflict(format!(
                "representative verification failed for {}",
                row.source_key
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
    use crate::model::{ActivityFilter, AttemptFilter};
    use async_trait::async_trait;
    use std::{
        collections::HashMap,
        sync::{
            Mutex,
            atomic::{AtomicI64, Ordering},
        },
    };

    #[derive(Default)]
    struct MemoryRepository {
        rows: Mutex<HashMap<String, Activity>>,
        counter: AtomicI64,
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
                id: id.unwrap(),
                input,
                legacy_attempt_id: legacy_id,
                created_at: now,
                updated_at: now,
            };
            self.upsert(activity.clone()).await?;
            Ok(activity)
        }
        async fn upsert(&self, activity: Activity) -> Result<bool, AppError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .insert(activity.id.clone(), activity)
                .is_none())
        }
        async fn get(&self, id: &str) -> Result<Option<Activity>, AppError> {
            Ok(self.rows.lock().unwrap().get(id).cloned())
        }
        async fn list(&self, _: &ActivityFilter) -> Result<Vec<Activity>, AppError> {
            Ok(Vec::new())
        }
        async fn count(&self, _: &ActivityFilter) -> Result<u64, AppError> {
            Ok(self.rows.lock().unwrap().len() as u64)
        }
        async fn replace(&self, _: &str, _: ActivityInput) -> Result<Option<Activity>, AppError> {
            Ok(None)
        }
        async fn delete(&self, _: &str) -> Result<bool, AppError> {
            Ok(false)
        }
        async fn list_attempts(&self, _: &AttemptFilter) -> Result<Vec<Activity>, AppError> {
            Ok(Vec::new())
        }
        async fn count_attempts(&self, _: &AttemptFilter) -> Result<u64, AppError> {
            Ok(0)
        }
        async fn get_attempt(&self, _: i64) -> Result<Option<Activity>, AppError> {
            Ok(None)
        }
        async fn next_attempt_id(&self) -> Result<i64, AppError> {
            Ok(self.counter.fetch_add(1, Ordering::SeqCst) + 1)
        }
        async fn seed_attempt_counter(&self, _: i64) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn sample_rows() -> Vec<(usize, InterviewStackRow)> {
        let input = concat!(
            r#"{"sourceKey":"interviewstack-001","date":"2026-09-01","company":"Netflix","role":"ML Engineer","level":"Entry","topic":"Algorithms","score":75,"type":"challenge","status":"complete"}"#,
            "\n",
            r#"{"sourceKey":"interviewstack-002","date":"2026-08-01","company":"Meta","role":"AI Engineer","level":"Entry","topic":"Vision","score":null,"type":"casual","status":"incomplete"}"#
        );
        parse_ndjson(input.as_bytes()).0
    }

    #[test]
    fn validates_completed_and_incomplete_score_rules() {
        let rows = sample_rows();
        assert_eq!(rows.len(), 2);
        let invalid = r#"{"sourceKey":"bad","date":"2026-09-01","company":"X","role":"Y","level":"Entry","topic":"Z","score":null,"type":"casual","status":"complete"}"#;
        let (_, failures) = parse_ndjson(invalid.as_bytes());
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn authoritative_fixture_has_expected_shape() {
        let fixture = include_str!("../fixtures/interviewstack_attempts.ndjson");
        let (rows, failures) = parse_ndjson(fixture.as_bytes());
        assert!(failures.is_empty());
        assert_eq!(rows.len(), 68);
        assert_eq!(
            rows.iter()
                .filter(|(_, row)| row.status == InterviewStackStatus::Complete)
                .count(),
            67
        );
        assert_eq!(
            rows.iter()
                .filter(|(_, row)| row.status == InterviewStackStatus::Incomplete
                    && row.score.is_none())
                .count(),
            1
        );
        assert_eq!(rows.first().unwrap().1.source_key, "interviewstack-001");
        assert_eq!(rows.last().unwrap().1.source_key, "interviewstack-068");
    }

    #[tokio::test]
    async fn rerun_skips_unchanged_rows_without_duplicates() {
        let repository: Arc<dyn ActivityRepository> = Arc::new(MemoryRepository::default());
        let rows = sample_rows();
        let first = import_rows(&rows, Vec::new(), 2, repository.clone())
            .await
            .unwrap();
        assert_eq!(
            (first.inserted, first.skipped, first.verified_count),
            (2, 0, 2)
        );
        let second = import_rows(&rows, Vec::new(), 2, repository.clone())
            .await
            .unwrap();
        assert_eq!(
            (
                second.inserted,
                second.updated,
                second.skipped,
                second.verified_count
            ),
            (0, 0, 2, 2)
        );
        assert_eq!(
            repository
                .count(&ActivityFilter {
                    limit: 100,
                    ..Default::default()
                })
                .await
                .unwrap(),
            2
        );
    }
}
