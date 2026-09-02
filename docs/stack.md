# Stack inventory

| Layer | Technology | Responsibility |
| --- | --- | --- |
| Language/runtime | Rust 2024 edition, Tokio | Type-safe service and asynchronous I/O |
| HTTP | Axum, Tower, tower-http | Routing, middleware, CORS, request IDs, static files |
| Data | Official MongoDB Rust driver | Connection pooling, BSON, indexes, and CRUD |
| Schemas | Serde and domain validation | camelCase JSON/BSON and business invariants |
| API docs | utoipa, utoipa-swagger-ui | OpenAPI 3 schema and vendored Swagger UI |
| Observability | tracing, tracing-subscriber | Structured request and application logs |
| Time/IDs | chrono, UUID | UTC timestamps and source-independent IDs |
| Migration | sqlx (PostgreSQL), clap | Legacy reader and command-line tools |
| UI | Static HTML, CSS, JavaScript | Interview dashboard and manual CRUD workflow |
| Packaging | Cargo, Docker, Docker Compose | Reproducible builds and local stack |
| CI | GitHub Actions | Format, Clippy, tests, and locked release build |

MongoDB is the only runtime persistence layer. PostgreSQL is solely an optional
migration source. There is no Redis, search cluster, graph database, Python
runtime, or LLM dependency in the serving path.
