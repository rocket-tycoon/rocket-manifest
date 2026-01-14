# Manifest justfile
# Run `just` to see available commands

# Default: list available commands
default:
    @just --list

# Build debug
build:
    cargo build

# Build release
release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run db specs only
test-db:
    cargo test db_spec

# Run api specs only
test-api:
    cargo test api_spec

# Run with cargo watch (requires cargo-watch)
watch:
    cargo watch -x test

# Start server on default port (3000)
serve:
    cargo run -- serve

# Start server on custom port
serve-port port="8080":
    cargo run -- serve -p {{port}}

# Check code without building
check:
    cargo check

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Format check (CI)
fmt-check:
    cargo fmt -- --check

# Clean build artifacts
clean:
    cargo clean

# Run all CI checks (format, lint, test)
ci: fmt-check lint test

# Publish a new version (bumps version, tags, pushes, triggers release workflow)
publish version:
    ./scripts/release.sh {{version}}
