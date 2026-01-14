# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs

# Build dependencies only
RUN cargo build --release && rm -rf src target/release/deps/manifest*

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
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/bash mfst

# Create data directory
RUN mkdir -p /data && chown mfst:mfst /data

# Copy binary from builder
COPY --from=builder /app/target/release/mfst /usr/local/bin/mfst

# Switch to non-root user
USER mfst

# Set data directory and bind to all interfaces for container networking
ENV MANIFEST_DATA_DIR=/data
ENV MANIFEST_BIND_ADDR=0.0.0.0

# Expose port
EXPOSE 17010

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:17010/api/v1/health || exit 1

# Run the server
ENTRYPOINT ["mfst"]
CMD ["serve", "--port", "17010"]
