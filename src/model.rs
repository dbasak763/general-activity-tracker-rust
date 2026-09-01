use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use mongodb::bson::{Bson, Document};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::error::AppError;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    LeetCode,
    Codeforces,
    LogicPuzzle,
    AiMlTopic,
    ResearchPaper,
    ModelExperiment,
    ProjectMilestone,
    JobApplication,
    NetworkingInteraction,
    Interview,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Planned,
    InProgress,
    #[default]
    Completed,
    Incomplete,
    Invalidated,
    Skipped,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeetCodeDetails {
    #[validate(length(min = 1, max = 200))]
    pub problem: String,
    #[validate(range(min = 1, max = 5000))]
    pub problem_number: Option<u32>,
    pub difficulty: ProblemDifficulty,
    pub language: Option<String>,
    #[validate(range(min = 0))]
    pub runtime_ms: Option<u64>,
    #[validate(range(min = 0))]
    pub memory_kb: Option<u64>,
    pub accepted: bool,
    #[serde(default)]
    pub techniques: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProblemDifficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodeforcesDetails {
    #[validate(length(min = 1, max = 60))]
    pub contest_id: String,
    #[validate(length(min = 1, max = 16))]
    pub problem_index: String,
    #[validate(range(min = 800, max = 4000))]
    pub rating: Option<u16>,
    pub verdict: String,
    pub language: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicPuzzleDetails {
    #[validate(length(min = 1, max = 120))]
    pub source: String,
    pub solved: bool,
    #[validate(range(min = 1))]
    pub attempts: Option<u32>,
    #[validate(length(max = 10000))]
    pub solution_summary: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiMlTopicDetails {
    #[validate(length(min = 1, max = 200))]
    pub topic: String,
    #[validate(length(max = 120))]
    pub learning_resource: Option<String>,
    #[validate(range(min = 0, max = 100))]
    pub mastery_percent: Option<u8>,
    #[serde(default)]
    pub concepts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchPaperDetails {
    #[validate(length(min = 1, max = 500))]
    pub paper_title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub publication_year: Option<u16>,
    pub paper_id: Option<String>,
    #[validate(range(min = 1))]
    pub pages_read: Option<u32>,
    #[validate(length(max = 20000))]
    pub key_takeaways: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelExperimentDetails {
    #[validate(length(min = 1, max = 200))]
    pub experiment_name: String,
    pub experiment_id: Option<String>,
    #[validate(length(min = 1, max = 120))]
    pub model_name: String,
    pub dataset: Option<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMilestoneDetails {
    pub project_id: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub milestone: String,
    #[validate(range(min = 0, max = 100))]
    pub completion_percent: u8,
    pub release: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobApplicationDetails {
    pub application_id: Option<String>,
    #[validate(length(min = 1, max = 150))]
    pub company: String,
    #[validate(length(min = 1, max = 150))]
    pub role: String,
    pub stage: ApplicationStage,
    pub location: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationStage {
    Saved,
    Applied,
    RecruiterScreen,
    Interview,
    Offer,
    Rejected,
    Withdrawn,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkingInteractionDetails {
    pub person_id: Option<String>,
    #[validate(length(min = 1, max = 150))]
    pub person_name: String,
    pub interaction_type: InteractionType,
    pub organization: Option<String>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub follow_up_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionType {
    Message,
    Call,
    Meeting,
    Event,
    Referral,
    FollowUp,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewDetails {
    #[validate(length(min = 1, max = 200))]
    pub topic: String,
    pub attempted_date: NaiveDate,
    #[serde(default)]
    pub attempt_source: AttemptSource,
    #[validate(length(max = 100))]
    pub external_attempt_id: Option<String>,
    #[validate(length(max = 36))]
    pub challenge_id: Option<String>,
    #[validate(length(max = 300))]
    pub challenge_title: Option<String>,
    #[validate(range(min = 1))]
    pub round_number: Option<u16>,
    #[validate(length(max = 250))]
    pub round_name: Option<String>,
    #[validate(length(max = 250))]
    pub focus_topic: Option<String>,
    #[validate(length(max = 200))]
    pub question_bank_topic_slug: Option<String>,
    #[validate(range(min = 1))]
    pub attempt_number: Option<u32>,
    #[validate(length(max = 150))]
    pub company: Option<String>,
    #[validate(length(max = 150))]
    pub role: Option<String>,
    #[validate(length(max = 100))]
    pub level: Option<String>,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub priority_next_drill: Option<String>,
    pub application_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AttemptSource {
    #[default]
    Manual,
    Casual,
    Challenge,
    QuestionBank,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyAttemptStatus {
    Incomplete,
    #[default]
    Complete,
    Invalidated,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivityDetails {
    LeetCode(LeetCodeDetails),
    Codeforces(CodeforcesDetails),
    LogicPuzzle(LogicPuzzleDetails),
    AiMlTopic(AiMlTopicDetails),
    ResearchPaper(ResearchPaperDetails),
    ModelExperiment(ModelExperimentDetails),
    ProjectMilestone(ProjectMilestoneDetails),
    JobApplication(JobApplicationDetails),
    NetworkingInteraction(NetworkingInteractionDetails),
    Interview(InterviewDetails),
}

impl ActivityDetails {
    pub fn activity_type(&self) -> ActivityType {
        match self {
            Self::LeetCode(_) => ActivityType::LeetCode,
            Self::Codeforces(_) => ActivityType::Codeforces,
            Self::LogicPuzzle(_) => ActivityType::LogicPuzzle,
            Self::AiMlTopic(_) => ActivityType::AiMlTopic,
            Self::ResearchPaper(_) => ActivityType::ResearchPaper,
            Self::ModelExperiment(_) => ActivityType::ModelExperiment,
            Self::ProjectMilestone(_) => ActivityType::ProjectMilestone,
            Self::JobApplication(_) => ActivityType::JobApplication,
            Self::NetworkingInteraction(_) => ActivityType::NetworkingInteraction,
            Self::Interview(_) => ActivityType::Interview,
        }
    }

    fn validate_payload(&self) -> Result<(), validator::ValidationErrors> {
        match self {
            Self::LeetCode(value) => value.validate(),
            Self::Codeforces(value) => value.validate(),
            Self::LogicPuzzle(value) => value.validate(),
            Self::AiMlTopic(value) => value.validate(),
            Self::ResearchPaper(value) => value.validate(),
            Self::ModelExperiment(value) => value.validate(),
            Self::ProjectMilestone(value) => value.validate(),
            Self::JobApplication(value) => value.validate(),
            Self::NetworkingInteraction(value) => value.validate(),
            Self::Interview(value) => value.validate(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInput {
    #[validate(length(min = 1, max = 120))]
    pub user_id: String,
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    #[validate(length(min = 1, max = 300))]
    pub title: String,
    #[validate(length(max = 10000))]
    pub description: Option<String>,
    #[validate(length(max = 120))]
    pub category: Option<String>,
    #[serde(default)]
    pub status: ActivityStatus,
    #[serde(default)]
    pub priority: Priority,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub planned_at: Option<DateTime<Utc>>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub completed_at: Option<DateTime<Utc>>,
    #[validate(range(min = 0))]
    pub duration_minutes: Option<u32>,
    pub notes: Option<String>,
    #[validate(range(min = 0.0, max = 100.0))]
    pub score: Option<f64>,
    #[validate(range(min = 0.0, max = 5.0))]
    pub rating: Option<f64>,
    pub feedback: Option<String>,
    #[validate(url)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entity_refs: BTreeMap<String, String>,
    pub details: ActivityDetails,
    #[serde(default)]
    pub metadata: Document,
}

impl ActivityInput {
    pub fn validate_domain(&self) -> Result<(), AppError> {
        self.validate()
            .map_err(|error| AppError::validation(error.to_string()))?;
        self.details
            .validate_payload()
            .map_err(|error| AppError::validation(error.to_string()))?;
        if self.activity_type != self.details.activity_type() {
            return Err(AppError::validation("type must match details.kind"));
        }
        if let (Some(start), Some(completed)) = (self.started_at, self.completed_at)
            && completed < start
        {
            return Err(AppError::validation(
                "completedAt cannot be earlier than startedAt",
            ));
        }
        if let ActivityDetails::Interview(interview) = &self.details {
            if self.status == ActivityStatus::Completed && self.score.is_none() {
                return Err(AppError::validation(
                    "a completed interview attempt must have a score",
                ));
            }
            if interview.attempt_source == AttemptSource::Challenge {
                let complete = interview.round_number.is_some()
                    && interview.round_name.is_some()
                    && interview.focus_topic.is_some()
                    && interview.attempt_number.is_some();
                if !complete {
                    return Err(AppError::validation(
                        "challenge attempts require roundNumber, roundName, focusTopic, and attemptNumber",
                    ));
                }
            }
        }
        if self.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(AppError::validation("tags cannot contain empty values"));
        }
        if self
            .metadata
            .keys()
            .any(|key| key.starts_with('$') || key.contains('.'))
        {
            return Err(AppError::validation(
                "metadata keys cannot start with '$' or contain '.'",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(flatten)]
    pub input: ActivityInput,
    pub legacy_attempt_id: Option<i64>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityFilter {
    pub user_id: Option<String>,
    #[serde(rename = "type")]
    pub activity_type: Option<ActivityType>,
    pub status: Option<ActivityStatus>,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptFilter {
    pub company: Option<String>,
    pub role: Option<String>,
    pub level: Option<String>,
    pub topic: Option<String>,
    pub attempt_source: Option<AttemptSource>,
    pub challenge_id: Option<String>,
    pub round_number: Option<u16>,
    pub status: Option<LegacyAttemptStatus>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

impl AttemptFilter {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.limit == 0 || self.limit > 500 {
            return Err(AppError::validation("limit must be between 1 and 500"));
        }
        if let (Some(start), Some(end)) = (self.start_date, self.end_date)
            && end < start
        {
            return Err(AppError::validation(
                "endDate cannot be earlier than startDate",
            ));
        }
        Ok(())
    }
}

fn default_limit() -> u64 {
    100
}

impl ActivityFilter {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.limit == 0 || self.limit > 500 {
            return Err(AppError::validation("limit must be between 1 and 500"));
        }
        if self.offset > 100_000 {
            return Err(AppError::validation("offset cannot exceed 100000"));
        }
        if let (Some(start), Some(end)) = (self.start_date, self.end_date)
            && end < start
        {
            return Err(AppError::validation(
                "endDate cannot be earlier than startDate",
            ));
        }
        Ok(())
    }
}

pub fn json_to_bson_document(value: serde_json::Value) -> Result<Document, AppError> {
    let bson = mongodb::bson::to_bson(&value)?;
    bson.as_document()
        .cloned()
        .ok_or_else(|| AppError::validation("metadata must be a JSON object"))
}

pub fn metadata_with_legacy_id(id: i64) -> Document {
    let mut metadata = Document::new();
    metadata.insert(
        "migrationSource",
        Bson::String("postgresql.interview_attempts".to_owned()),
    );
    metadata.insert("legacyId", Bson::Int64(id));
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_leetcode() -> ActivityInput {
        ActivityInput {
            user_id: "user-1".to_owned(),
            activity_type: ActivityType::LeetCode,
            title: "Two Sum".to_owned(),
            description: None,
            category: Some("coding".to_owned()),
            status: ActivityStatus::Completed,
            priority: Priority::Medium,
            planned_at: None,
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            duration_minutes: Some(15),
            notes: None,
            score: Some(100.0),
            rating: None,
            feedback: None,
            source_url: Some("https://leetcode.com/problems/two-sum/".to_owned()),
            tags: vec!["arrays".to_owned()],
            entity_refs: BTreeMap::new(),
            details: ActivityDetails::LeetCode(LeetCodeDetails {
                problem: "Two Sum".to_owned(),
                problem_number: Some(1),
                difficulty: ProblemDifficulty::Easy,
                language: Some("rust".to_owned()),
                runtime_ms: Some(1),
                memory_kb: Some(2048),
                accepted: true,
                techniques: vec!["hash-map".to_owned()],
            }),
            metadata: Document::new(),
        }
    }

    #[test]
    fn validates_representative_leetcode_activity() {
        valid_leetcode().validate_domain().unwrap();
    }

    #[test]
    fn rejects_mismatched_type_and_details() {
        let mut input = valid_leetcode();
        input.activity_type = ActivityType::ResearchPaper;
        assert!(
            input
                .validate_domain()
                .unwrap_err()
                .to_string()
                .contains("must match")
        );
    }

    #[test]
    fn rejects_invalid_completion_order() {
        let mut input = valid_leetcode();
        input.started_at = Some(Utc::now());
        input.completed_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(input.validate_domain().is_err());
    }
}
