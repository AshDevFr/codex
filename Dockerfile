# Multi-stage Dockerfile for Codex

# Stage 1: Build dependencies
FROM rust:1.92-alpine AS chef
RUN apk add --no-cache \
    musl-dev \
    pkgconf \
    openssl-dev \
    openssl-libs-static \
    build-base \
    curl
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Prepare recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build dependencies (cached layer)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Install build dependencies
RUN apk add --no-cache \
    pkgconf \
    openssl-dev \
    openssl-libs-static \
    musl-dev \
    build-base \
    curl

# Build dependencies (this layer is cached)
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 4: Build application
COPY . .
ENV OPENSSL_STATIC=1
RUN cargo build --release

# Stage 5: Runtime
FROM alpine:latest AS runtime

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    openssl

# Create app user
RUN adduser -D -u 1000 codex

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/codex /usr/local/bin/codex

# Create directories
RUN mkdir -p /app/data /app/config && \
    chown -R codex:codex /app

USER codex

EXPOSE 8080

CMD ["codex", "serve"]
