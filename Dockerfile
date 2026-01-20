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
RUN apk add --no-cache \
    musl-dev \
    build-base
RUN cargo install cargo-chef
WORKDIR /app

# Stage 3: Prepare recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build dependencies (cached layer)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

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

# Install runtime dependencies (ca-certificates for HTTPS, su-exec for user switching)
RUN apk add --no-cache ca-certificates su-exec

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
