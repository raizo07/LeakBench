.PHONY: all build test fmt clippy demo web serve clean

all: test

build:
	cargo build --release

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# Regenerate both scenarios, including the bundle the UI imports.
demo:
	cargo run -p leakbench-cli -- demo --out reports --web web/src/demo

# Rebuild the report UI into a single self-contained index.html.
web:
	cd web && pnpm install && pnpm build

serve:
	cargo run -p leakbench-cli -- serve --dir reports --static web/dist

clean:
	cargo clean
	rm -rf reports
