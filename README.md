# General Activity Tracker in Rust

A production-oriented Axum service for tracking learning, research, projects,
career activity, networking, and interviews in MongoDB. It is a clean Rust and
MongoDB migration of the public
[`fastapi-pg-interview-tracker`](https://github.com/dbasak763/fastapi-pg-interview-tracker),
while broadening interview attempts into one module of a general timeline.

## What it tracks

One `activities` collection stores a shared envelope and one validated subtype:

- LeetCode and Codeforces submissions
- logic puzzles and AI/ML study topics
- research-paper reading and model experiments
- project milestones
- job applications and networking interactions
- interviews, including sessions/rounds, focus topics, scores, strengths,
  feedback, and priority next drills

Reusable entities live separately in `users`, `projects`, `papers`, `topics`,
`experiments`, `companies`, `applications`, `people`, and `interviews`.
Activities refer to them through `entityRefs`; there is deliberately no
collection per activity subtype, Neo4j, Redis, or OpenSearch.

## Quick start

The easiest local setup uses the same connection string for the service and
MongoDB Compass:

```bash
docker compose up --build
curl http://localhost:8080/health/ready
```

Connection string:

```text
mongodb://activity:activity@localhost:27017/activity_tracker?authSource=admin
```

For a local Rust run, copy `.env.example` to `.env`, start MongoDB, then run:

```bash
cargo run --bin activity-tracker
```

Important configuration:

| Variable | Default | Purpose |
| --- | --- | --- |
| `MONGODB_URI` | required | Service and Compass connection string |
| `MONGODB_DATABASE` | `activity_tracker` | Database name |
| `APP_HOST` / `APP_PORT` | `0.0.0.0` / `8080` | Bind address |
| `CORS_ALLOWED_ORIGINS` | local ports 3000 and 5173 | Comma-separated exact origins |
| `RUST_LOG` | service and HTTP info | `tracing` filter |
| `JSON_LOGS` | `false` | JSON logs when true |

No secrets are checked in. Change the Compose credentials outside local use.

## API

The main timeline endpoints are:

```text
POST   /api/activities
GET    /api/activities
GET    /api/activities/count
GET    /api/activities/{id}
PUT    /api/activities/{id}
DELETE /api/activities/{id}
```

Filters include `userId`, `type`, `status`, `category`, `tag`, `startDate`,
`endDate`, `limit`, and `offset`. `details.kind` must match the top-level
`type`; timestamp ordering, scores, ratings, subtype fields, URLs, tags, and
metadata keys are validated before persistence.

The original `/api/attempts` CRUD/filter/count/latest routes and score dashboard
routes remain available with camelCase JSON and numeric compatibility IDs. See
[API compatibility](docs/api-compatibility.md) for exact behavior and the few
intentional changes.

Representative payloads for every supported type are in
[`examples/activities.json`](examples/activities.json).

## PostgreSQL migration

The migrator reads the original `interview_attempts` table and maps every
column. Start with a source-only dry run:

```bash
DATABASE_URL='postgresql://user:password@localhost/interviews' \
  cargo run --bin migrate-postgres -- --dry-run
```

Then write to MongoDB:

```bash
DATABASE_URL='postgresql://user:password@localhost/interviews' \
MONGODB_URI='mongodb://activity:activity@localhost:27017/activity_tracker?authSource=admin' \
  cargo run --bin migrate-postgres
```

Every row uses `_id = legacy:interview_attempts:<id>` and preserves the numeric
ID in `legacyAttemptId` and `metadata.legacyId`. Writes are upserts, so reruns
replace the same records rather than duplicating them. The command validates
all mappings before writing, seeds the numeric compatibility counter above the
largest imported ID, and reads back representative first/middle/last records.
The JSON report distinguishes inserts from updates and lists the source/mapped
count. Full field mapping and recovery steps are in
[the migration guide](docs/postgres-migration.md).

## MongoDB Compass

1. Open Compass and paste the same `MONGODB_URI` used by the service.
2. Select `activity_tracker`, then `activities`.
3. Use List view for scanning the timeline, JSON view for subtype details, and
   Table view for common fields. Schema analysis shows shared and subtype
   distributions.
4. Inspect the Indexes tab for user/time, type/time, tags, compatibility IDs,
   paper, Codeforces, and challenge-attempt indexes.

Useful Compass filters:

```javascript
{ type: "leetcode", "details.accepted": true }
{ type: "codeforces", "details.rating": { $gte: 1600 } }
{ type: "research_paper", "details.publicationYear": { $gte: 2024 } }
{ type: "interview", "details.company": "Example Co", score: { $gte: 80 } }
{ tags: "rust", startedAt: { $gte: ISODate("2026-01-01") } }
```

## Quality gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --release
```

Unit tests cover domain and subtype validation, legacy mapping, canonical topic
behavior, repository filter construction, and HTTP behavior. MongoDB-backed
smoke checks can be run against the Compose stack. See
[architecture and data flow](docs/architecture.md) for the boundaries.

