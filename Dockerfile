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
FROM rust:1.92-alpine AS chef
# clang is needed for some proc-macro build scripts (proc-macro2, quote, etc.)
RUN apk add --no-cache \
    musl-dev \
    build-base \
    clang
RUN cargo install cargo-chef
WORKDIR /app

# Stage 3: Prepare recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build dependencies (cached layer)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Disable static linking to enable dlopen() for PDFium dynamic loading
# This is required because musl's static linking doesn't support dlopen()
ENV RUSTFLAGS="-C target-feature=-crt-static"

# Build dependencies (this layer is cached)
RUN cargo chef cook --release --features embed-frontend --recipe-path recipe.json

# Stage 5: Build application
COPY . .

# Copy frontend dist from frontend-builder
COPY --from=frontend-builder /web/dist ./web/dist

# Build with embedded frontend
RUN cargo build --release --features embed-frontend

# Stage 6: Runtime
FROM alpine:latest AS runtime

# Install runtime dependencies:
# - ca-certificates for HTTPS
# - su-exec for user switching
# - libstdc++ and libgcc for PDFium
RUN apk add --no-cache ca-certificates su-exec libstdc++ libgcc

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

# Copy binary from builder
COPY --from=builder /app/target/release/codex /usr/local/bin/codex

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Create directories
RUN mkdir -p /app/data /app/config && \
    chown -R codex:codex /app

# Environment variables for user mapping (defaults)
ENV PUID=1000
ENV PGID=1000

EXPOSE 8080

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["codex", "serve"]
