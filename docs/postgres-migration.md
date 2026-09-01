# PostgreSQL to MongoDB migration

## Field map

| PostgreSQL column | MongoDB field |
| --- | --- |
| `id` | `legacyAttemptId`, `metadata.legacyId`, and deterministic `_id` |
| `attempted_date` | `details.attemptedDate` |
| `attempt_source` | `details.attemptSource` |
| `external_attempt_id` | `details.externalAttemptId` |
| `source_url` | `sourceUrl` |
| `challenge_id` | `details.challengeId` |
| `challenge_title` | `details.challengeTitle` and preferred `title` |
| `round_number` | `details.roundNumber` |
| `round_name` | `details.roundName` |
| `focus_topic` | `details.focusTopic` |
| `question_bank_topic_slug` | `details.questionBankTopicSlug` |
| `attempt_number` | `details.attemptNumber` |
| `company` | `details.company` |
| `role` | `details.role` |
| `level` | `details.level` |
| `topic` | `details.topic` and fallback `title` |
| `score` | `score` as a floating-point percentage |
| `status` | `status` (`complete` becomes `completed`) |
| `notes` | `notes` |
| `started_at` | `startedAt` |
| `completed_at` | `completedAt` and derived `durationMinutes` |
| `created_at` | `createdAt` and initial `updatedAt` |

The migrator also sets `userId: "legacy"`, `type: "interview"`,
`category: "interview"`, `details.kind: "interview"`, and migration tags.

## Procedure

1. Back up PostgreSQL and MongoDB using the tools appropriate to your hosting
   environment. The migrator never modifies PostgreSQL.
2. Run `--dry-run`. This opens only PostgreSQL, reads all rows in ID order,
   validates every mapped document, and prints counts without requiring MongoDB.
3. Run without `--dry-run` using the target `MONGODB_URI`.
4. Compare `sourceCount` and `mappedCount`. On a first run,
   `insertedCount + updatedCount` must equal `sourceCount`.
5. A successful report includes up to three verified samples. Check MongoDB
   counts independently:

   ```javascript
   db.activities.countDocuments({ "metadata.migrationSource": "postgresql.interview_attempts" })
   db.activities.find({ _id: "legacy:interview_attempts:1" })
   db.activities.find({ type: "interview" }).sort({ legacyAttemptId: -1 }).limit(5)
   ```

If a run is interrupted, rerun the same command. Deterministic IDs and upserts
make it restartable. A rerun should report updates instead of additional
inserts. Correct bad source data in PostgreSQL and rerun; do not hand-edit ID
mappings. The migrator seeds the counter to the largest imported ID so newly
created compatibility attempts do not collide.

The migration expects the upgraded source schema present in the public source
repository. No private credentials are required by the code; operators supply
their own `DATABASE_URL` and `MONGODB_URI` at execution time.
