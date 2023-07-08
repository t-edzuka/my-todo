test:
    cargo test

fmt:
    cargo clippy
    cargo fmt --all

watch:
    cargo watch -x run

audit:
    cargo deny check advisories

all:fmt fix test audit

fix:
    cargo fix && cargo clippy --fix

fix-force:
    cargo fix --allow-dirty && cargo clippy --fix --allow-dirty

