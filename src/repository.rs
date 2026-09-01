use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use futures::TryStreamExt;
use mongodb::{
    Client, Collection, Database, IndexModel,
    bson::{Bson, Document, doc, to_bson},
    options::{IndexOptions, ReturnDocument},
};

use crate::{
    error::AppError,
    model::{Activity, ActivityFilter, ActivityInput, AttemptFilter},
};

#[async_trait]
pub trait ActivityRepository: Send + Sync {
    async fn ping(&self) -> Result<(), AppError>;
    async fn ensure_indexes(&self) -> Result<(), AppError>;
    async fn create(
        &self,
        input: ActivityInput,
        id: Option<String>,
        legacy_id: Option<i64>,
    ) -> Result<Activity, AppError>;
    async fn upsert(&self, activity: Activity) -> Result<bool, AppError>;
    async fn get(&self, id: &str) -> Result<Option<Activity>, AppError>;
    async fn list(&self, filter: &ActivityFilter) -> Result<Vec<Activity>, AppError>;
    async fn count(&self, filter: &ActivityFilter) -> Result<u64, AppError>;
    async fn replace(&self, id: &str, input: ActivityInput) -> Result<Option<Activity>, AppError>;
    async fn delete(&self, id: &str) -> Result<bool, AppError>;
    async fn list_attempts(&self, filter: &AttemptFilter) -> Result<Vec<Activity>, AppError>;
    async fn count_attempts(&self, filter: &AttemptFilter) -> Result<u64, AppError>;
    async fn get_attempt(&self, legacy_id: i64) -> Result<Option<Activity>, AppError>;
    async fn next_attempt_id(&self) -> Result<i64, AppError>;
    async fn seed_attempt_counter(&self, minimum: i64) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct MongoActivityRepository {
    database: Database,
    activities: Collection<Activity>,
    counters: Collection<Document>,
}

impl MongoActivityRepository {
    pub async fn connect(uri: &str, database_name: &str) -> Result<Self, AppError> {
        let client = Client::with_uri_str(uri).await?;
        let database = client.database(database_name);
        Ok(Self {
            activities: database.collection("activities"),
            counters: database.collection("counters"),
            database,
        })
    }

    fn activity_filter(filter: &ActivityFilter) -> Result<Document, AppError> {
        let mut query = Document::new();
        if let Some(value) = &filter.user_id {
            query.insert("userId", value);
        }
        if let Some(value) = &filter.activity_type {
            query.insert("type", to_bson(value)?);
        }
        if let Some(value) = &filter.status {
            query.insert("status", to_bson(value)?);
        }
        if let Some(value) = &filter.category {
            query.insert("category", value);
        }
        if let Some(value) = &filter.tag {
            query.insert("tags", value);
        }
        add_date_window(&mut query, "startedAt", filter.start_date, filter.end_date);
        Ok(query)
    }

    fn attempt_filter(filter: &AttemptFilter) -> Result<Document, AppError> {
        let mut query = doc! { "type": "interview", "details.kind": "interview" };
        for (key, value) in [
            ("details.company", filter.company.as_ref()),
            ("details.role", filter.role.as_ref()),
            ("details.level", filter.level.as_ref()),
            ("details.topic", filter.topic.as_ref()),
            ("details.challengeId", filter.challenge_id.as_ref()),
        ] {
            if let Some(value) = value {
                query.insert(key, value);
            }
        }
        if let Some(value) = &filter.attempt_source {
            query.insert("details.attemptSource", to_bson(value)?);
        }
        if let Some(value) = filter.round_number {
            query.insert("details.roundNumber", i32::from(value));
        }
        if let Some(value) = &filter.status {
            let stored = match value {
                crate::model::LegacyAttemptStatus::Incomplete => "incomplete",
                crate::model::LegacyAttemptStatus::Complete => "completed",
                crate::model::LegacyAttemptStatus::Invalidated => "invalidated",
            };
            query.insert("status", stored);
        }
        add_naive_date_window(
            &mut query,
            "details.attemptedDate",
            filter.start_date,
            filter.end_date,
        );
        Ok(query)
    }
}

fn add_date_window(
    query: &mut Document,
    field: &str,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) {
    let mut range = Document::new();
    if let Some(start) = start.and_then(|d| d.and_hms_opt(0, 0, 0)) {
        range.insert(
            "$gte",
            mongodb::bson::DateTime::from_millis(
                DateTime::<Utc>::from_naive_utc_and_offset(start, Utc).timestamp_millis(),
            ),
        );
    }
    if let Some(end) = end
        .and_then(|d| d.succ_opt())
        .and_then(|d| d.and_hms_opt(0, 0, 0))
    {
        range.insert(
            "$lt",
            mongodb::bson::DateTime::from_millis(
                DateTime::<Utc>::from_naive_utc_and_offset(end, Utc).timestamp_millis(),
            ),
        );
    }
    if !range.is_empty() {
        query.insert(field, range);
    }
}

fn add_naive_date_window(
    query: &mut Document,
    field: &str,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) {
    let mut range = Document::new();
    if let Some(start) = start {
        range.insert("$gte", start.format("%Y-%m-%d").to_string());
    }
    if let Some(end) = end {
        range.insert("$lte", end.format("%Y-%m-%d").to_string());
    }
    if !range.is_empty() {
        query.insert(field, range);
    }
}

#[async_trait]
impl ActivityRepository for MongoActivityRepository {
    async fn ping(&self) -> Result<(), AppError> {
        self.database.run_command(doc! { "ping": 1 }).await?;
        Ok(())
    }

    async fn ensure_indexes(&self) -> Result<(), AppError> {
        let indexes = vec![
            IndexModel::builder().keys(doc! { "userId": 1, "startedAt": -1 }).build(),
            IndexModel::builder().keys(doc! { "userId": 1, "type": 1, "startedAt": -1 }).build(),
            IndexModel::builder().keys(doc! { "tags": 1 }).build(),
            IndexModel::builder().keys(doc! { "entityRefs.$**": 1 }).build(),
            IndexModel::builder().keys(doc! { "legacyAttemptId": 1 }).options(IndexOptions::builder().unique(true).sparse(true).name("ux_legacy_attempt_id".to_owned()).build()).build(),
            IndexModel::builder().keys(doc! { "details.externalAttemptId": 1 }).options(IndexOptions::builder().unique(true).sparse(true).name("ux_external_attempt_id".to_owned()).build()).build(),
            IndexModel::builder().keys(doc! { "details.challengeId": 1, "details.roundNumber": 1, "details.focusTopic": 1, "details.attemptNumber": 1 }).options(IndexOptions::builder().unique(true).partial_filter_expression(doc! { "details.kind": "interview", "details.attemptSource": "challenge", "details.challengeId": { "$exists": true }, "details.roundNumber": { "$exists": true }, "details.focusTopic": { "$exists": true }, "details.attemptNumber": { "$exists": true } }).name("ux_challenge_attempt_sequence".to_owned()).build()).build(),
            IndexModel::builder().keys(doc! { "details.paperId": 1 }).options(IndexOptions::builder().sparse(true).build()).build(),
            IndexModel::builder().keys(doc! { "details.contestId": 1, "details.problemIndex": 1 }).build(),
        ];
        self.activities.create_indexes(indexes).await?;
        for name in [
            "users",
            "projects",
            "papers",
            "topics",
            "experiments",
            "companies",
            "applications",
            "people",
            "interviews",
        ] {
            let collection = self.database.collection::<Document>(name);
            collection
                .create_index(IndexModel::builder().keys(doc! { "updatedAt": -1 }).build())
                .await?;
        }
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
            id: id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            input,
            legacy_attempt_id: legacy_id,
            created_at: now,
            updated_at: now,
        };
        self.activities.insert_one(&activity).await?;
        Ok(activity)
    }

    async fn upsert(&self, activity: Activity) -> Result<bool, AppError> {
        let result = self
            .activities
            .replace_one(doc! { "_id": &activity.id }, &activity)
            .upsert(true)
            .await?;
        Ok(result.upserted_id.is_some())
    }

    async fn get(&self, id: &str) -> Result<Option<Activity>, AppError> {
        Ok(self.activities.find_one(doc! { "_id": id }).await?)
    }

    async fn list(&self, filter: &ActivityFilter) -> Result<Vec<Activity>, AppError> {
        let cursor = self
            .activities
            .find(Self::activity_filter(filter)?)
            .sort(doc! { "startedAt": -1, "_id": -1 })
            .skip(filter.offset)
            .limit(filter.limit as i64)
            .await?;
        Ok(cursor.try_collect().await?)
    }

    async fn count(&self, filter: &ActivityFilter) -> Result<u64, AppError> {
        Ok(self
            .activities
            .count_documents(Self::activity_filter(filter)?)
            .await?)
    }

    async fn replace(&self, id: &str, input: ActivityInput) -> Result<Option<Activity>, AppError> {
        let Some(existing) = self.get(id).await? else {
            return Ok(None);
        };
        let activity = Activity {
            id: id.to_owned(),
            input,
            legacy_attempt_id: existing.legacy_attempt_id,
            created_at: existing.created_at,
            updated_at: Utc::now(),
        };
        self.activities
            .replace_one(doc! { "_id": id }, &activity)
            .await?;
        Ok(Some(activity))
    }

    async fn delete(&self, id: &str) -> Result<bool, AppError> {
        Ok(self
            .activities
            .delete_one(doc! { "_id": id })
            .await?
            .deleted_count
            == 1)
    }

    async fn list_attempts(&self, filter: &AttemptFilter) -> Result<Vec<Activity>, AppError> {
        let cursor = self
            .activities
            .find(Self::attempt_filter(filter)?)
            .sort(doc! { "startedAt": -1, "legacyAttemptId": -1 })
            .skip(filter.offset)
            .limit(filter.limit as i64)
            .await?;
        Ok(cursor.try_collect().await?)
    }

    async fn count_attempts(&self, filter: &AttemptFilter) -> Result<u64, AppError> {
        Ok(self
            .activities
            .count_documents(Self::attempt_filter(filter)?)
            .await?)
    }

    async fn get_attempt(&self, legacy_id: i64) -> Result<Option<Activity>, AppError> {
        Ok(self
            .activities
            .find_one(doc! { "legacyAttemptId": legacy_id })
            .await?)
    }

    async fn next_attempt_id(&self) -> Result<i64, AppError> {
        let counter = self
            .counters
            .find_one_and_update(
                doc! { "_id": "interview_attempts" },
                doc! { "$inc": { "sequence": 1_i64 } },
            )
            .upsert(true)
            .return_document(ReturnDocument::After)
            .await?;
        counter
            .and_then(|doc| match doc.get("sequence") {
                Some(Bson::Int64(v)) => Some(*v),
                Some(Bson::Int32(v)) => Some(i64::from(*v)),
                _ => None,
            })
            .ok_or_else(|| AppError::Conflict("could not allocate interview attempt ID".to_owned()))
    }

    async fn seed_attempt_counter(&self, minimum: i64) -> Result<(), AppError> {
        self.counters
            .update_one(
                doc! { "_id": "interview_attempts" },
                doc! { "$max": { "sequence": minimum } },
            )
            .upsert(true)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActivityStatus, ActivityType, LegacyAttemptStatus};

    #[test]
    fn builds_indexable_activity_filter() {
        let query = MongoActivityRepository::activity_filter(&ActivityFilter {
            user_id: Some("user-1".to_owned()),
            activity_type: Some(ActivityType::Interview),
            status: Some(ActivityStatus::Completed),
            tag: Some("rust".to_owned()),
            limit: 25,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(query.get_str("userId").unwrap(), "user-1");
        assert_eq!(query.get_str("type").unwrap(), "interview");
        assert_eq!(query.get_str("status").unwrap(), "completed");
        assert_eq!(query.get_str("tags").unwrap(), "rust");
    }

    #[test]
    fn maps_legacy_complete_filter_to_stored_completed_status() {
        let query = MongoActivityRepository::attempt_filter(&AttemptFilter {
            company: Some("Example Co".to_owned()),
            status: Some(LegacyAttemptStatus::Complete),
            start_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            end_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            limit: 100,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(query.get_str("type").unwrap(), "interview");
        assert_eq!(query.get_str("details.company").unwrap(), "Example Co");
        assert_eq!(query.get_str("status").unwrap(), "completed");
        let range = query.get_document("details.attemptedDate").unwrap();
        assert_eq!(range.get_str("$gte").unwrap(), "2026-01-01");
        assert_eq!(range.get_str("$lte").unwrap(), "2026-12-31");
    }
}
