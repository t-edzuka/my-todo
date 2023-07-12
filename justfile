run:
    RUST_LOG=debug cargo run

# test with real postgresql database using .env file that contains DATABASE_URL
test-e2e:
    RUST_LOG=debug cargo test

# standalone in-memory test
test:
    RUST_LOG=debug cargo test --no-default-features

fmt:
    cargo clippy
    cargo fmt --all


# It will start by running cargo check.
# If it succeeds, it launches cargo test.
# If tests pass, it launches the application with cargo run.
watch:
     RUST_LOG=debug cargo watch -x check -x test -x run

audit:
    cargo deny check advisories

all:fmt fix test audit

fix:
    cargo fix --allow-staged && cargo clippy --fix --allow-staged

fix-force:
    cargo fix --allow-dirty && cargo clippy --fix --allow-dirty

