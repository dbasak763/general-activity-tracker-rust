# Testing

## Offline quality gates

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --release
```

Tests cover domain and subtype validation, compatibility mapping, repository
filters and indexes, HTTP CRUD and validation, OpenAPI and static-dashboard
contracts, NDJSON validation, and idempotent imports through an in-memory
repository.

## Live MongoDB smoke test

Start MongoDB, then launch the app against a dedicated test database:

```bash
docker compose up -d mongodb
MONGODB_URI='mongodb://activity:activity@127.0.0.1:27017/activity_tracker?authSource=admin' \
MONGODB_DATABASE='activity_tracker_test' \
APP_HOST='127.0.0.1' APP_PORT='18080' \
cargo run --locked --bin activity-tracker
```

In another shell:

```bash
curl -fsS http://127.0.0.1:18080/health/live
curl -fsS http://127.0.0.1:18080/health/ready
curl -fsS http://127.0.0.1:18080/api-doc/openapi.json
curl -fsS http://127.0.0.1:18080/api/attempts/count
```

Open `http://127.0.0.1:18080/dashboard` to exercise manual entry and
`http://127.0.0.1:18080/docs/` to exercise the same POST through Swagger UI.
Use a dedicated test database and remove only synthetic records after recording
their returned IDs.

Import verification:

```bash
MONGODB_URI='mongodb://127.0.0.1:27017' \
MONGODB_DATABASE='activity_tracker_test' \
cargo run --locked --bin import-interviewstack -- \
  --input fixtures/interviewstack_attempts.ndjson
```

Run it twice. The first report should insert 68, the second should skip 68, and
both write runs should report `failed: 0`, `verifiedCount: 68`, and three
verified representative samples.
