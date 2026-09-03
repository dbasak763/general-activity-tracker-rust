FROM rust:1.98-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --locked --release --bin activity-tracker

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/activity-tracker /usr/local/bin/activity-tracker
COPY static ./static
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["activity-tracker"]
