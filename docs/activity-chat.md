# Activity chat

## Component 1: explicit relationship storage

MongoDB remains the source of truth for activities and user-confirmed links.
`POST /api/relationships` accepts `sourceId`, `targetId`, `kind`, and a `reason`.
Kinds are `related_to`, `addresses`, `applies`, `inspired_by`, `part_of`, and
`prerequisite_for`. The direction is source → target. Both activities must exist;
self-links, unknown kinds, and empty reasons are rejected. Repeated saves of the
same source/kind/target update that relationship instead of duplicating it.

`GET /api/relationships?activityId=...` reads explicit links, and
`DELETE /api/relationships/{id}` removes one. Removing an activity removes its
explicit links. The GET list is bounded to 2000 links. These routes follow the
existing local, single-user API model; user IDs are filters, not authentication.

Derived links based on common concepts or shared entities are separate from
explicit links. Sharing a topic is evidence of association, not proof of cause,
mastery, or a learning weakness. A model suggestion must be confirmed before it
is saved as an explicit link.

Run `python3 scripts/test-relationships.py` against a test Rust server on port
18080 (or set `TEST_API_URL`). The script verifies persistence, idempotency,
validation, deletion and cleanup, then removes only its own synthetic records.
