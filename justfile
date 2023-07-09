run:
    RUST_LOG=debug cargo run
test:
    RUST_LOG=debug cargo test

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
    cargo fix && cargo clippy --fix

fix-force:
    cargo fix --allow-dirty && cargo clippy --fix --allow-dirty

