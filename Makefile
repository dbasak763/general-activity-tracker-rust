.PHONY: check test run compose-up compose-down migrate-dry-run

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test --all-targets --all-features

test:
	cargo test --all-targets --all-features

run:
	cargo run --bin activity-tracker

compose-up:
	docker compose up --build

compose-down:
	docker compose down

migrate-dry-run:
	cargo run --bin migrate-postgres -- --dry-run

