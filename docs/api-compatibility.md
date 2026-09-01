# API compatibility

The original FastAPI frontend can continue using the following routes:

| Original route | Rust status |
| --- | --- |
| `GET /` | Preserved route; message now names the general tracker |
| `GET /database-health` | Preserved; now pings MongoDB |
| `POST/GET /api/attempts` | Preserved camelCase payloads and status codes |
| `GET /api/attempts/count` | Preserved exact, unpaginated count |
| `GET /api/attempts/latest` | Preserved nullable newest result |
| `GET/PUT/DELETE /api/attempts/{id}` | Preserved numeric IDs and 204 delete |
| `/api/dashboard/score-history` | Preserved grouped chronological scores |
| `/api/dashboard/score-timeline` | Preserved scored attempt timeline |
| `/api/dashboard/topics` | Preserved canonical topic counts |
| `/api/dashboard/topic-summaries` | Preserved score summaries |
| `/api/dashboard/topic-score-progression` | Preserved topic progression |
| `GET /api/dashboard/chat/config` | Preserved shape; reports deterministic fallback |
| `POST /api/dashboard/chat` | Preserved response shape; database-backed count fallback only |

Query aliases remain camelCase: `attemptSource`, `challengeId`, `roundNumber`,
`startDate`, and `endDate`. JSON fields use camelCase. Existing interview IDs
remain numeric through `legacyAttemptId`; migrated IDs are preserved exactly.

Intentional changes:

- PostgreSQL is not used at runtime. `/database-health` returns the configured
  MongoDB database name (normally `activity_tracker`).
- Python/LangGraph and third-party LLM calls are not included. The former chat
  endpoint returns bounded, verified database counts. This keeps normal CRUD
  entirely in Rust and avoids inventing an internal AI service.
- FastAPI's generated Swagger UI is not reproduced. The Rust routes and example
  payloads are documented in this repository.
- Errors add a stable machine-readable `code` alongside the compatible
  `detail` field.
- A new `/api/activities` API exposes the broader product model, and
  `/health/live` plus `/health/ready` split process and dependency health.

