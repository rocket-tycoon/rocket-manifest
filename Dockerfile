# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs

# Build dependencies only
RUN cargo build --release && rm -rf src target/release/deps/rocket_manifest*

# Copy actual source
COPY src ./src

# Build the real binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/bash rmf

# Create data directory
RUN mkdir -p /data && chown rmf:rmf /data

# Copy binary from builder
COPY --from=builder /app/target/release/rmf /usr/local/bin/rmf

# Switch to non-root user
USER rmf

# Set data directory
ENV ROCKET_MANIFEST_DATA_DIR=/data

# Expose port
EXPOSE 17010

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:17010/api/v1/health || exit 1

# Run the server
ENTRYPOINT ["rmf"]
CMD ["serve", "--port", "17010"]
