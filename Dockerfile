# Multi-stage Dockerfile for Codex

# Stage 1: Build frontend
FROM node:22-alpine AS frontend-builder
WORKDIR /web

# Copy package files
# COPY web/package*.json ./
COPY web/package.json ./

# Install dependencies
RUN npm install

# Copy frontend source
COPY web/ ./

# Build frontend
RUN npm run build

# Stage 2: Rust build dependencies
FROM rust:1.95-alpine AS chef
# clang is needed for some proc-macro build scripts (proc-macro2, quote, etc.)
RUN apk add --no-cache \
    musl-dev \
    build-base \
    clang \
    mold
RUN cargo install cargo-chef
WORKDIR /app

# Compiler flags:
# - target-feature=-crt-static: Disable static linking for PDFium dlopen() support
# - linker=clang + fuse-ld=mold: Use mold linker for faster linking
ENV RUSTFLAGS="-C target-feature=-crt-static -C linker=clang -C link-arg=-fuse-ld=mold"

# Stage 3: Prepare recipe
FROM chef AS planner
# Only copy Rust-related files to avoid cache invalidation from frontend changes
COPY Cargo.toml Cargo.lock ./
COPY assets/ ./assets/
COPY migration/ ./migration/
COPY src/ ./src/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build dependencies (cached layer)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies (this layer is cached)
# Use BuildKit cache mounts to persist Cargo registry/git between builds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --features embed-frontend --recipe-path recipe.json

# Stage 5: Build application
# Only copy Rust-related files to avoid cache invalidation from frontend changes
COPY Cargo.toml Cargo.lock ./
COPY assets/ ./assets/
COPY migration/ ./migration/
COPY src/ ./src/

# Copy frontend dist from frontend-builder
COPY --from=frontend-builder /web/dist ./web/dist

# Build with embedded frontend
# Cache target directory for incremental compilation
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --features embed-frontend && \
    cp /app/target/release/codex /app/codex

# Stage 6: Runtime
FROM alpine:latest AS runtime

# Install runtime dependencies:
# - ca-certificates for HTTPS
# - su-exec for user switching
# - libstdc++ and libgcc for PDFium
# - nodejs and npm for TypeScript/JavaScript plugins
RUN apk add --no-cache ca-certificates su-exec libstdc++ libgcc curl wget nodejs npm

# Install uv (fast Python package manager) for Python plugins
# uv provides 'uvx' command for running Python packages without installation
RUN wget -qO- https://astral.sh/uv/install.sh | sh && \
    mv /root/.local/bin/uv /usr/local/bin/uv && \
    mv /root/.local/bin/uvx /usr/local/bin/uvx

# Install PDFium library for PDF page rendering
# This enables rendering of text-only and vector PDF pages
# Using musl build for Alpine Linux compatibility
# Note: Downloads architecture-specific binary based on TARGETARCH
ARG TARGETARCH
ARG PDFIUM_VERSION=latest
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        wget -q -O- https://github.com/bblanchon/pdfium-binaries/releases/${PDFIUM_VERSION}/download/pdfium-linux-musl-arm64.tgz \
        | tar -xz -C /usr/local; \
    else \
        wget -q -O- https://github.com/bblanchon/pdfium-binaries/releases/${PDFIUM_VERSION}/download/pdfium-linux-musl-x64.tgz \
        | tar -xz -C /usr/local; \
    fi

# Ensure PDFium library can be found at runtime
# Create symlink in /usr/lib for standard library search path
RUN ln -sf /usr/local/lib/libpdfium.so /usr/lib/libpdfium.so
ENV LD_LIBRARY_PATH=/usr/local/lib:/usr/lib

# Create app user with default UID/GID (can be changed at runtime via PUID/PGID)
RUN addgroup -g 1000 codex && \
    adduser -D -u 1000 -G codex codex

WORKDIR /app

# Copy binary from builder (copied out of cache mount during build)
COPY --from=builder /app/codex /usr/local/bin/codex

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Create directories
# - /app/data, /app/config: Application data and config
# - /app/.npm: npm cache for npx plugins (avoids permission issues with /.npm)
# Set permissions to allow any UID to write (for custom user: directive in docker-compose)
RUN mkdir -p /app/data /app/config /app/.npm && \
    chown -R codex:codex /app && \
    chmod 777 /app/data /app/config /app/.npm

# Set npm cache location to avoid permission issues when running as non-root
# This is needed for npx-based plugins to download packages
ENV npm_config_cache=/app/.npm

# Environment variables for user mapping (defaults)
ENV PUID=1000
ENV PGID=1000

EXPOSE 8080

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["codex", "serve"]
