# Makefile for Codex development and deployment

.PHONY: help build test run dev-* test-* docs-* docker-* db-*

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
NC := \033[0m # No Color

help: ## Show this help message
	@echo "$(BLUE)Codex Development Commands$(NC)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-22s$(NC) %s\n", $$1, $$2}'

# =============================================================================
# Development Environment (Docker)
# =============================================================================

dev-up: ## Start development environment
	docker compose --profile dev up

dev-up-d: ## Start development environment (detached)
	docker compose --profile dev up -d

dev-up-build: ## Start development environment with rebuild
	docker compose --profile dev up --build

dev-down: ## Stop development environment
	docker compose --profile dev down

dev-down-v: ## Stop development environment and remove volumes
	docker compose --profile dev down -v

dev-logs: ## View all development logs
	docker compose logs -f codex-dev codex-dev-worker frontend-dev

dev-logs-backend: ## View backend logs only
	docker compose logs -f codex-dev

dev-logs-worker: ## View worker logs only
	docker compose logs -f codex-dev-worker

dev-logs-frontend: ## View frontend logs only
	docker compose logs -f frontend-dev

dev-restart: ## Restart all development containers
	docker compose restart codex-dev codex-dev-worker frontend-dev

dev-restart-backend: ## Restart backend only
	docker compose restart codex-dev codex-dev-worker

dev-restart-frontend: ## Restart frontend only
	docker compose restart frontend-dev

dev-shell: ## Open shell in backend container
	docker exec -it codex-dev sh

dev-shell-frontend: ## Open shell in frontend container
	docker exec -it codex-frontend-dev sh

dev-seed: ## Create initial admin user in dev environment
	docker compose --profile dev exec codex-dev cargo run -- seed --config config/config.docker.yaml

dev-watch: ## Start with watch mode (auto-sync code changes)
	docker compose -f docker-compose.yml -f compose.watch.yml --profile dev watch

# =============================================================================
# Testing
# =============================================================================

test: ## Run backend tests (SQLite)
	cargo test

test-frontend: ## Run frontend tests
	cd web && npm run test:run

test-all: ## Run all tests (frontend + backend with PostgreSQL)
	@echo "$(YELLOW)Running frontend tests...$(NC)"
	cd web && npm run test:run
	@echo "$(GREEN)Frontend tests complete!$(NC)"
	@$(MAKE) test-postgres

test-postgres: ## Run PostgreSQL tests only
	@$(MAKE) test-up
	@$(MAKE) test-postgres-run
	@$(MAKE) test-down

test-postgres-run: ## Run PostgreSQL tests (assumes DB is running)
	@echo "$(YELLOW)Waiting for PostgreSQL to be ready...$(NC)"
	@until docker exec codex-postgres-test pg_isready -U codex_test > /dev/null 2>&1; do sleep 1; done
	@echo "$(GREEN)PostgreSQL is ready!$(NC)"
	POSTGRES_HOST=localhost POSTGRES_PORT=5433 POSTGRES_USER=codex_test POSTGRES_PASSWORD=codex_test POSTGRES_DB=codex_test \
		cargo test --features rar -- --include-ignored

test-up: ## Start test database
	docker compose --profile test up -d postgres-test

test-down: ## Stop test database
	docker compose --profile test down

test-clean: ## Stop test database and remove volumes
	docker compose --profile test down -v

# =============================================================================
# Documentation (Docusaurus)
# =============================================================================

docs-install: ## Install documentation dependencies
	cd docs && npm install

docs-start: ## Start documentation dev server
	cd docs && npm start

docs-start-fresh: ## Start documentation dev server with fresh API docs
	@$(MAKE) docs-clean-api-docs
	@$(MAKE) docs-gen-api-docs
	@$(MAKE) docs-start

docs-build: ## Build documentation for production
	cd docs && npm run build

docs-build-fresh: ## Build documentation for production with fresh API docs
	@$(MAKE) docs-clean-api-docs
	@$(MAKE) docs-gen-api-docs
	@$(MAKE) docs-build

docs-build-docker: ## Build documentation for production in Docker
	docker compose --profile docs build

docs-serve: ## Serve built documentation locally
	cd docs && npm run serve

docs-clear: ## Clear documentation cache
	cd docs && npm run clear

docs-gen-api-docs: ## Generate API docs
	@$(MAKE) openapi
	cd docs && npm run gen-api-docs

docs-clean-api-docs: ## Clean API docs
	cd docs && npm run clean-api-docs

# =============================================================================
# Local Development (without Docker)
# =============================================================================

build: ## Build the project
	cargo build

build-release: ## Build with release optimizations
	cargo build --release

run: ## Run the application locally
	RUST_LOG=info cargo run

watch: ## Run with hot reload (requires cargo-watch)
	RUST_LOG=info cargo watch -x run

# =============================================================================
# Frontend Development
# =============================================================================

frontend: ## Start frontend dev server (requires backend)
	cd web && npm run dev

frontend-mock: ## Start frontend with mock API (no backend needed)
	cd web && npm run dev:mock

frontend-mock-fresh: openapi-all ## Regenerate API types, then start frontend with mocks
	cd web && npm run dev:mock

frontend-install: ## Install frontend dependencies
	cd web && npm install

openapi: ## Generate OpenAPI spec from backend
	cargo run -- openapi --output web/openapi.json

openapi-types: ## Generate TypeScript types from OpenAPI spec
	cd web && npm run generate:types

openapi-all: openapi openapi-types ## Generate OpenAPI spec and TypeScript types

frontend-fixtures: ## Generate mock fixture files (CBZ, EPUB, PDF)
	cd web && npm run generate:fixtures

frontend-lint: ## Run frontend lint
	cd web && npm run lint

# =============================================================================
# Setup
# =============================================================================

setup-hooks: ## Install pre-commit hooks
	@command -v pre-commit >/dev/null 2>&1 || { echo "$(YELLOW)Installing pre-commit...$(NC)"; pip install pre-commit; }
	pre-commit install
	@echo "$(GREEN)Pre-commit hooks installed!$(NC)"

# =============================================================================
# Code Quality
# =============================================================================

fmt: ## Format code
	cargo fmt

lint: ## Run clippy
	cargo clippy -- -D warnings

check: fmt lint test ## Run format, lint, and tests

ci: ## Run CI checks (format check, lint, tests, build)
	cargo fmt -- --check
	cargo clippy -- -D warnings
	cargo check --features embed-frontend
	cargo test
	cd web && npm run test:run
	cargo build --release

# =============================================================================
# Database Management
# =============================================================================

db-seed: ## Create initial admin user (SQLite)
	cargo run -- seed --config config/config.sqlite.yaml

db-shell: ## Open PostgreSQL shell (production)
	docker exec -it codex-postgres psql -U codex -d codex

db-shell-test: ## Open PostgreSQL shell (test)
	docker exec -it codex-postgres-test psql -U codex_test -d codex_test

db-backup: ## Backup production database
	@mkdir -p backups
	docker exec codex-postgres pg_dump -U codex codex > backups/codex-backup-$$(date +%Y%m%d-%H%M%S).sql
	@echo "$(GREEN)Backup created in backups/$(NC)"

db-restore: ## Restore database (usage: make db-restore BACKUP_FILE=file.sql)
	@if [ -z "$(BACKUP_FILE)" ]; then \
		echo "$(YELLOW)Error: BACKUP_FILE not set. Use: make db-restore BACKUP_FILE=backups/file.sql$(NC)"; \
		exit 1; \
	fi
	cat $(BACKUP_FILE) | docker exec -i codex-postgres psql -U codex codex

# =============================================================================
# Docker Production
# =============================================================================

docker-build: ## Build production Docker image
	docker build -t codex:latest .

docker-run: ## Run production container
	docker run -p 8080:8080 codex:latest

docker-push: ## Push to registry (usage: make docker-push REGISTRY=your-registry)
	@if [ -z "$(REGISTRY)" ]; then \
		echo "$(YELLOW)Error: REGISTRY not set. Use: make docker-push REGISTRY=your-registry$(NC)"; \
		exit 1; \
	fi
	docker tag codex:latest $(REGISTRY)/codex:latest
	docker push $(REGISTRY)/codex:latest

# =============================================================================
# Production Compose
# =============================================================================

prod-up: ## Start production services
	docker compose --profile prod up

prod-up-d: ## Start production services (detached)
	docker compose --profile prod up -d

prod-down: ## Stop production services
	docker compose --profile prod down

prod-logs: ## View production logs
	docker compose logs -f

# =============================================================================
# Cleanup
# =============================================================================

clean: ## Clean build artifacts
	cargo clean

clean-docker: ## Remove Docker containers and images
	docker compose --profile dev --profile test --profile prod down
	docker rmi codex:latest codex:dev 2>/dev/null || true

clean-all: clean clean-docker ## Clean everything (artifacts + Docker + volumes)
	docker compose --profile dev --profile test --profile prod down -v

# =============================================================================
# Binary Distribution (cargo-dist)
# =============================================================================

dist-install: ## Install cargo-dist tool
	cargo install cargo-dist --locked

dist-plan: ## Plan what cargo-dist will build (dry run)
	cargo dist plan

dist-build: ## Build standalone binaries for all platforms
	cargo dist build

dist-build-local: ## Build binary for current platform only
	cargo dist build --local

# =============================================================================
# Changelog (git-cliff)
# =============================================================================

changelog: ## Generate changelog from git history
	git-cliff -o CHANGELOG.md

changelog-unreleased: ## Show unreleased changes (preview)
	git-cliff --unreleased

changelog-release: ## Generate changelog for a new release (usage: make changelog-release VERSION=1.0.0)
	@if [ -z "$(VERSION)" ]; then \
		echo "$(YELLOW)Error: VERSION not set. Use: make changelog-release VERSION=1.0.0$(NC)"; \
		exit 1; \
	fi
	git-cliff --tag v$(VERSION) -o CHANGELOG.md
	@echo "$(GREEN)Changelog generated for v$(VERSION)$(NC)"
