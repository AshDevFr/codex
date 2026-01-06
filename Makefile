# Makefile for Codex development and deployment

.PHONY: help build test run docker-* compose-*

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
NC := \033[0m # No Color

help: ## Show this help message
	@echo "$(BLUE)Codex Development Commands$(NC)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2}'

## Development

build: ## Build the project
	cargo build

build-release: ## Build with release optimizations
	cargo build --release

build-release-no-rar: ## Build with release optimizations without rar support
	cargo build --release --no-default-features

build-release-docker: ## Build with release optimizations and rar support for Docker
	docker build -f Dockerfile -t codex:latest .

test: ## Run all tests (SQLite only)
	cargo test

test-all: ## Run all tests including PostgreSQL
	@echo "$(YELLOW)Starting PostgreSQL test container...$(NC)"
	docker compose --profile test up -d postgres-test
	@echo "$(YELLOW)Waiting for PostgreSQL to be ready...$(NC)"
	@until docker exec codex-postgres-test pg_isready -U codex_test > /dev/null 2>&1; do \
		sleep 1; \
	done
	@echo "$(GREEN)PostgreSQL is ready!$(NC)"
	@echo "$(GREEN)Running all tests with PostgreSQL support...$(NC)"
	@POSTGRES_HOST=localhost POSTGRES_PORT=5433 POSTGRES_USER=codex_test POSTGRES_PASSWORD=codex_test POSTGRES_DB=codex_test \
		cargo test --features rar -- --include-ignored; \
		TEST_EXIT_CODE=$$?; \
		echo "$(GREEN)Stopping test container...$(NC)"; \
		docker compose --profile test down; \
		exit $$TEST_EXIT_CODE

test-postgres: ## Run PostgreSQL tests (requires running PG instance)
	@echo "$(YELLOW)Starting PostgreSQL test container...$(NC)"
	docker compose --profile test up -d postgres-test
	@echo "$(YELLOW)Waiting for PostgreSQL to be ready...$(NC)"
	@until docker exec codex-postgres-test pg_isready -U codex_test > /dev/null 2>&1; do \
		sleep 1; \
	done
	@echo "$(GREEN)PostgreSQL is ready!$(NC)"
	@echo "$(GREEN)Running PostgreSQL integration tests...$(NC)"
	POSTGRES_HOST=localhost POSTGRES_PORT=5433 POSTGRES_USER=codex_test POSTGRES_PASSWORD=codex_test POSTGRES_DB=codex_test \
		cargo test --test postgres_integration_tests -- --ignored
	@echo "$(GREEN)Running PostgreSQL unit tests...$(NC)"
	POSTGRES_HOST=localhost POSTGRES_PORT=5433 POSTGRES_USER=codex_test POSTGRES_PASSWORD=codex_test POSTGRES_DB=codex_test \
		cargo test db::postgres -- --ignored
	@echo "$(GREEN)Stopping test container...$(NC)"
	docker compose --profile test down

run: ## Run the application locally
	RUST_LOG=info cargo run

watch: ## Run with hot reload
	RUST_LOG=info cargo watch -x run

clean: ## Clean build artifacts
	cargo clean
	rm -rf target/

## Docker - Production

docker-build: ## Build production Docker image
	docker build -t codex:latest .

docker-run: ## Run production container
	docker run -p 8080:8080 codex:latest

docker-push: ## Push to registry (set REGISTRY env var)
	@if [ -z "$(REGISTRY)" ]; then \
		echo "$(YELLOW)Error: REGISTRY not set. Use: make docker-push REGISTRY=your-registry$(NC)"; \
		exit 1; \
	fi
	docker tag codex:latest $(REGISTRY)/codex:latest
	docker push $(REGISTRY)/codex:latest

## Docker Compose

compose-up: ## Start all services (production mode)
	docker compose --profile prod up

compose-up-d: ## Start all services (production mode)
	docker compose --profile prod up -d

compose-down: ## Stop all services
	docker compose --profile prod --profile dev --profile test down

compose-down-v: ## Stop all services and remove volumes
	docker compose --profile prod --profile dev --profile test down -v

compose-logs: ## View logs
	docker compose logs -f

compose-ps: ## Show running services
	docker compose ps

compose-restart: ## Restart services
	docker compose restart

## Docker Compose - Development

dev-up: ## Start development environment with hot reload
	docker compose --profile dev up

dev-up-d: ## Start development environment with hot reload
	docker compose --profile dev up -d

dev-down: ## Stop development environment
	docker compose --profile dev down

dev-down-v: ## Stop development environment and remove volumes
	docker compose --profile dev down -v

dev-logs: ## View development logs (backend + frontend)
	docker compose logs -f codex-dev frontend-dev

dev-logs-backend: ## View backend logs only
	docker compose logs -f codex-dev

dev-logs-frontend: ## View frontend logs only
	docker compose logs -f frontend-dev

dev-watch: ## Start with watch mode (auto-sync code changes for backend + frontend)
	docker compose -f docker-compose.yml -f compose.watch.yml --profile dev watch

dev-watch-backend: ## Watch backend only
	docker compose -f docker-compose.yml -f compose.watch.yml --profile dev watch codex-dev

dev-watch-frontend: ## Watch frontend only
	docker compose -f docker-compose.yml -f compose.watch.yml --profile dev watch frontend-dev

dev-shell: ## Open shell in backend development container
	docker exec -it codex-dev sh

dev-shell-frontend: ## Open shell in frontend development container
	docker exec -it codex-frontend-dev sh

dev-restart: ## Restart development containers (backend + frontend)
	docker compose restart codex-dev frontend-dev

dev-restart-backend: ## Restart backend only
	docker compose restart codex-dev

dev-restart-frontend: ## Restart frontend only
	docker compose restart frontend-dev

## Docker Compose - Testing

test-up: ## Start test database
	docker compose --profile test up -d postgres-test

test-down: ## Stop test database
	docker compose --profile test down

test-clean: ## Clean test data
	docker compose --profile test down -v

## Database Management

db-shell: ## Open PostgreSQL shell (production)
	docker exec -it codex-postgres psql -U codex -d codex

db-shell-test: ## Open PostgreSQL shell (test)
	docker exec -it codex-postgres-test psql -U codex_test -d codex_test

db-backup: ## Backup production database
	@mkdir -p backups
	docker exec codex-postgres pg_dump -U codex codex > backups/codex-backup-$$(date +%Y%m%d-%H%M%S).sql
	@echo "$(GREEN)Backup created in backups/$(NC)"

db-restore: ## Restore database (set BACKUP_FILE env var)
	@if [ -z "$(BACKUP_FILE)" ]; then \
		echo "$(YELLOW)Error: BACKUP_FILE not set. Use: make db-restore BACKUP_FILE=backups/file.sql$(NC)"; \
		exit 1; \
	fi
	cat $(BACKUP_FILE) | docker exec -i codex-postgres psql -U codex codex

db-reset: ## Reset database (DELETES ALL DATA!)
	@echo "$(YELLOW)WARNING: This will delete all data!$(NC)"
	@read -p "Are you sure? [y/N] " -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		docker compose down -v; \
		docker compose up -d; \
		echo "$(GREEN)Database reset complete$(NC)"; \
	else \
		echo "$(GREEN)Cancelled$(NC)"; \
	fi

db-seed: ## Create initial admin user and API key
	cargo run -- seed --config config/config.sqlite.yaml

db-seed-release: ## Create initial admin user using release build
	./target/release/codex seed --config config/config.sqlite.yaml

## Maintenance

clean-docker: ## Remove Docker images and containers
	docker compose --profile dev --profile test down
	docker rmi codex:latest codex:dev || true

clean-all: clean clean-docker ## Clean everything (build artifacts and Docker)
	docker compose --profile dev --profile test down -v
	docker system prune -f

fmt: ## Format code
	cargo fmt

lint: ## Run clippy
	cargo clippy -- -D warnings

check: fmt lint test ## Run all checks (format, lint, test)

## CI/CD

ci: ## Run CI checks
	cargo fmt -- --check
	cargo clippy -- -D warnings
	cargo test
	cargo build --release

## Binary Distribution (cargo-dist)

dist-install: ## Install cargo-dist tool
	@echo "$(BLUE)Installing cargo-dist...$(NC)"
	cargo install cargo-dist --locked

dist-plan: ## Plan what cargo-dist will build (dry run)
	cargo dist plan

dist-build: ## Build standalone binaries for all platforms locally
	@echo "$(BLUE)Building standalone binaries...$(NC)"
	cargo dist build

dist-build-local: ## Build binaries for current platform only
	@echo "$(BLUE)Building binary for current platform...$(NC)"
	cargo dist build --local

dist-release: ## Create a release (requires git tag)
	@echo "$(YELLOW)This will create a release. Make sure you've tagged the version first!$(NC)"
	cargo dist release

dist-test: ## Test the dist build process
	cargo dist plan --no-local

## Quick Actions

quick-dev: dev-up dev-logs ## Quick start development (start and follow logs)

quick-test: test-up test-postgres test-down ## Quick run PostgreSQL tests

quick-reset: compose-down compose-up compose-logs ## Quick reset (stop, start, logs)
