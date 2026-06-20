.PHONY: build release test lint fmt check schema update-deps clean install release-patch release-minor release-major

build:
	cargo build

release:
	cargo build --release

test:
	cargo nextest run

lint:
	cargo fmt -- --check
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

check: lint test schema

# Verify the agent contract is emitted and is valid JSON.
schema: build
	./target/debug/dotpick schema | python3 -c 'import json,sys; json.load(sys.stdin)' && echo "schema OK"

update-deps:
	upd --apply --max-bump minor --lang rust,actions

clean:
	cargo clean

install: release
	mkdir -p ~/.local/bin
	cp target/release/dotpick ~/.local/bin/dotpick

release-patch:
	vership bump patch

release-minor:
	vership bump minor

release-major:
	vership bump major
