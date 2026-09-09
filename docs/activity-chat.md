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

## Component 2: LangGraph, providers and Neo4j

The internal Python service compiles a LangGraph workflow: classify → build the
relationship view → lookup / analysis / relationships / visualization → answer.
Lookup prefers Ollama (`qwen3:8b`); other routes prefer Groq
(`openai/gpt-oss-120b`). All model and route preferences are environment settings.
Each request tries each configured provider at most once, with two bounded tool
rounds and an overall deadline. The labelled database fallback never pretends to
be an AI recommendation.

Models may call only `search_activities`, `summarize_activities`, and
`related_activities`. Arguments are validated, and the worker has no MongoDB
credentials. It receives a bounded snapshot from Rust. Exact snapshot coverage
travels with results; partial histories are labelled. Cloud calls receive only
selected tool results, the question, and up to ten recent conversation messages.
Citations and proposed relationship endpoints must belong to retrieved records.
The worker token and model keys stay server-side.

Neo4j stores a rebuildable projection of activities, concepts and entities. Tags,
techniques, concepts and topics connect activities; `metadata.weaknesses` supplies
explicit weakness assertions; entity references and company fields connect career
and project records. A free-form note alone does not automatically establish a
weakness. Explicit links are marked `user_confirmed`, while field-derived links
are marked `recorded_field`. Labels are normalized by whitespace and case only;
semantic synonyms require matching tags or a confirmed relationship.

Parameterized, server-owned Cypher performs up to four relationship hops.
Projection updates are transactional and scoped. A complete snapshot removes
obsolete projected edges/nodes; partial snapshots never purge unseen history.
Every query is also restricted to current snapshot IDs and edges, so deleted
records cannot return as evidence. When Neo4j is absent or unavailable, the same
recorded relationships are traversed in memory and the response says `derived`.

Run the worker locally:

```bash
python3 -m venv work/chat-venv
work/chat-venv/bin/pip install -r ai_service/requirements.txt
docker compose --profile ai up -d neo4j
CHAT_INTERNAL_TOKEN=activity-chat-local-development \
NEO4J_URI=bolt://127.0.0.1:7687 NEO4J_PASSWORD=activity-graph-local \
work/chat-venv/bin/python scripts/run-ai-service.py --provider-env /path/to/existing-tracker/.env
```

The optional `--provider-env` reads only Groq/Ollama routing settings as data,
without copying or printing credentials. Start the installed Ollama app first.
For containers, `docker compose --profile ai up -d --build ai neo4j` uses `.env`
settings. Default passwords/tokens are for local development; the worker and graph
ports bind to localhost. Do not expose this single-user app publicly as-is.

Unit tests: `work/chat-venv/bin/python -m unittest ai_service.test_workflow -v`.
Optional live tests: `scripts/test-ai-service.py --provider local` or
`--provider groq --provider-env /path/to/.env --neo4j`, run with the venv Python.
The live tests send synthetic records only; `--neo4j` tests projection cleanup and
scope isolation as well as the model's use of graph evidence.

Primary implementation references: [LangGraph workflows](https://docs.langchain.com/oss/python/langgraph/workflows-agents),
[Ollama compatibility](https://docs.ollama.com/api/openai-compatibility),
[Groq tool calling](https://console.groq.com/docs/tool-use/local-tool-calling),
[Neo4j parameterized queries](https://neo4j.com/docs/python-manual/current/query-simple/).

## Component 3: Rust chat boundary

The browser calls only `POST /api/dashboard/chat`. Rust validates the question and
up to ten history messages, reads up to `CHAT_MAX_ACTIVITIES` records from MongoDB,
loads confirmed relationships, and sends that snapshot to the internal worker.
`CHAT_INTERNAL_TOKEN` and model credentials are never accepted from or returned to
the browser. Requests are bounded by `CHAT_TIMEOUT_SECONDS`, worker responses have
a one MiB size limit, and response fields and collection sizes are checked before
they reach the UI.

`GET /api/dashboard/chat/config` reports the worker's provider routes without
exposing secrets. When the worker cannot be reached, the public endpoint remains
usable and returns a clearly labelled database-only count by activity type. A
partial snapshot includes explicit coverage metadata instead of implying that the
answer includes unseen history.

With the full stack running, `python3 scripts/test-chat-endpoint.py` creates two
synthetic activities and a confirmed link through Rust, asks a cross-activity
question through the public endpoint, verifies model citations and graph paths,
and removes its test records.
