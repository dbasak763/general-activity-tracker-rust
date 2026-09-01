# Architecture and data flow

## Runtime

```text
browser / API client
        |
        | HTTP + camelCase JSON
        v
Axum routes -> domain validation -> ActivityRepository -> MongoDB
        |                                  |
        |                                  +-- activities (timeline/source of truth)
        |                                  +-- counters (legacy numeric IDs)
        |                                  +-- reusable entity collections
        +-- Tower CORS, request IDs, tracing, structured errors
```

Tokio drives request concurrency and MongoDB I/O. Route code never constructs
database clients per request: `MongoActivityRepository` owns the shared driver
pool. All write payloads pass shared-envelope and subtype validation before the
repository boundary. MongoDB remains the single runtime source of truth.

## Activity document

The common envelope contains `_id`, `userId`, `type`, `title`, `description`,
`category`, `status`, `priority`, `plannedAt`, `startedAt`, `completedAt`,
`durationMinutes`, `notes`, `score`, `rating`, `feedback`, `sourceUrl`, `tags`,
`entityRefs`, `metadata`, `createdAt`, and `updatedAt`.

`details` contains a required `kind` discriminator and validated fields for one
of the ten supported types. The API rejects mismatched `type` and `details.kind`.
This gives the timeline predictable fields while leaving `metadata` available
for safe, forward-compatible annotations.

Reusable entity state belongs in separate collections. For example, a paper's
bibliography belongs in `papers`; a reading activity points to it through
`entityRefs.paper`. An interview may point to `applications` without requiring
a graph database. Add a graph store only after concrete multi-hop query and
latency requirements exist.

## Availability and operations

- `/health/live` only proves the process/event loop responds.
- `/health/ready` and `/database-health` issue a MongoDB `ping` and return an
  error if persistence is unavailable.
- Startup establishes connectivity and creates idempotent indexes before the
  listener accepts traffic.
- CORS is an exact environment-controlled allowlist.
- request IDs propagate through `x-request-id`; `tracing` can emit text or JSON.
- errors have a stable `{ "detail": ..., "code": ... }` body and avoid leaking
  database details to clients.

## Migration flow

```text
PostgreSQL interview_attempts
        |
        | ordered read, explicit column mapping
        v
validated Activity documents
        |
        | dry-run OR deterministic _id upsert
        v
MongoDB activities -> counter seed -> representative read-back validation
```

The process is safely restartable. It does not delete source rows, require
private production access, or write to the original application repository.

