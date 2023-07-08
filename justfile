run:
    RUST_LOG=debug cargo run
test:
    RUST_LOG=debug cargo test

fmt:
    cargo clippy
    cargo fmt --all

watch:
    RUST_LOG=debug cargo watch -x run

audit:
    cargo deny check advisories

all:fmt fix test audit

fix:
    cargo fix && cargo clippy --fix

fix-force:
    cargo fix --allow-dirty && cargo clippy --fix --allow-dirty

