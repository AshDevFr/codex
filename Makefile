# Makefile for Codex development and deployment

.PHONY: help build test run dev-* test-* docs-* docker-* db-* screenshots screenshots-* plugins-*

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

dev-check: ## Check development tool installation
	@echo "$(BLUE)Checking development tools...$(NC)"
	@echo ""
	@echo "$(BLUE)Required:$(NC)"
	@command -v cargo >/dev/null 2>&1 && echo "  $(GREEN)✓ cargo$(NC)" || echo "  $(YELLOW)✗ cargo (install from https://rustup.rs)$(NC)"
	@command -v node >/dev/null 2>&1 && echo "  $(GREEN)✓ node$(NC)" || echo "  $(YELLOW)✗ node (install from https://nodejs.org)$(NC)"
	@command -v docker >/dev/null 2>&1 && echo "  $(GREEN)✓ docker$(NC)" || echo "  $(YELLOW)✗ docker (install Docker Desktop)$(NC)"
	@echo ""
	@echo "$(BLUE)Optional (for faster builds):$(NC)"
	@command -v lld >/dev/null 2>&1 && echo "  $(GREEN)✓ lld$(NC) (faster linker)" || echo "  $(YELLOW)✗ lld$(NC) - install: brew install lld"
	@command -v mold >/dev/null 2>&1 && echo "  $(GREEN)✓ mold$(NC) (faster linker)" || echo "  $(YELLOW)✗ mold$(NC) - install: apt install mold (Linux only)"
	@cargo nextest --version >/dev/null 2>&1 && echo "  $(GREEN)✓ cargo-nextest$(NC) (faster tests)" || echo "  $(YELLOW)✗ cargo-nextest$(NC) - install: cargo install cargo-nextest --locked"

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

# Fast tests using cargo-nextest (parallel execution)
# Install: cargo install cargo-nextest --locked

test-fast: ## Run backend tests with nextest (faster, parallel)
	cargo nextest run

test-fast-all: ## Run all tests with nextest (faster, parallel)
	@echo "$(YELLOW)Running frontend tests...$(NC)"
	cd web && npm run test:run
	@echo "$(GREEN)Frontend tests complete!$(NC)"
	@$(MAKE) test-fast-postgres

test-fast-postgres: ## Run PostgreSQL tests with nextest
	@$(MAKE) test-up
	@$(MAKE) test-fast-postgres-run
	@$(MAKE) test-down

test-fast-postgres-run: ## Run PostgreSQL tests with nextest (assumes DB running)
	@echo "$(YELLOW)Waiting for PostgreSQL to be ready...$(NC)"
	@until docker exec codex-postgres-test pg_isready -U codex_test > /dev/null 2>&1; do sleep 1; done
	@echo "$(GREEN)PostgreSQL is ready!$(NC)"
	POSTGRES_HOST=localhost POSTGRES_PORT=5433 POSTGRES_USER=codex_test POSTGRES_PASSWORD=codex_test POSTGRES_DB=codex_test \
		cargo nextest run --features rar --run-ignored all

# =============================================================================
# Documentation (Docusaurus)
# =============================================================================

docs-install: ## Install documentation dependencies
	cd docs && npm install

docs-start: ## Start documentation dev server
	cd docs && npm start

docs-start-fresh: ## Start documentation dev server with fresh API docs
	@$(MAKE) docs-refresh-api-docs
	@$(MAKE) docs-start

docs-build: ## Build documentation for production
	cd docs && npm run build

docs-build-fresh: ## Build documentation for production with fresh API docs
	@$(MAKE) docs-refresh-api-docs
	@$(MAKE) docs-build

docs-build-docker: ## Build documentation for production in Docker
	docker compose --profile docs build

docs-serve: ## Serve built documentation locally
	cd docs && npm run serve

docs-clear: ## Clear documentation cache
	cd docs && npm run clear

docs-refresh-api-docs: ## Refresh API docs
	@$(MAKE) docs-clean-api-docs
	@$(MAKE) docs-gen-api-docs

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

frontend-build: ## Build frontend
	cd web && npm run build

frontend-mock: ## Start frontend with mock API (no backend needed)
	cd web && npm run dev:mock

frontend-mock-fresh: openapi-all ## Regenerate API types, then start frontend with mocks
	cd web && npm run dev:mock

frontend-install: ## Install frontend dependencies
	cd web && npm install

openapi: ## Generate OpenAPI spec from backend
	cargo run -- openapi --output web/openapi.json
	@echo "$(GREEN)OpenAPI spec generated!$(NC)"
	@echo "$(YELLOW)Copying OpenAPI spec to docs/api/openapi.json...$(NC)"
	cp web/openapi.json docs/api/openapi.json
	@echo "$(GREEN)OpenAPI spec copied to docs/api/openapi.json!$(NC)"


openapi-types: ## Generate TypeScript types from OpenAPI spec
	cd web && npm run generate:types

openapi-all: openapi openapi-types ## Generate OpenAPI spec and TypeScript types

frontend-fixtures: ## Generate mock fixture files (CBZ, EPUB, PDF)
	cd web && npm run generate:fixtures

frontend-lint: ## Run frontend lint
	cd web && npm run lint

frontend-lint-fix: ## Run frontend lint with auto-fix
	cd web && npm run lint -- --write

# =============================================================================
# Plugin Development
# =============================================================================

PLUGIN_DIRS := sdk-typescript metadata-echo metadata-mangabaka

plugins-install: ## Install dependencies for all plugins
	@echo "$(BLUE)Installing plugin dependencies...$(NC)"
	@for dir in $(PLUGIN_DIRS); do \
		echo "$(YELLOW)Installing $$dir...$(NC)"; \
		(cd plugins/$$dir && npm install); \
	done
	@echo "$(GREEN)All plugin dependencies installed!$(NC)"

plugins-build: ## Build all plugins
	@echo "$(BLUE)Building plugins...$(NC)"
	@echo "$(YELLOW)Building sdk-typescript...$(NC)"
	@cd plugins/sdk-typescript && npm run build
	@echo "$(YELLOW)Building metadata-echo...$(NC)"
	@cd plugins/metadata-echo && npm run build
	@echo "$(YELLOW)Building metadata-mangabaka...$(NC)"
	@cd plugins/metadata-mangabaka && npm run build
	@echo "$(GREEN)All plugins built!$(NC)"

plugins-lint: ## Run lint on all plugins
	@echo "$(BLUE)Linting plugins...$(NC)"
	@for dir in $(PLUGIN_DIRS); do \
		echo "$(YELLOW)Linting $$dir...$(NC)"; \
		(cd plugins/$$dir && npm run lint) || exit 1; \
	done
	@echo "$(GREEN)All plugins linted!$(NC)"

plugins-lint-fix: ## Run lint with auto-fix on all plugins
	@echo "$(BLUE)Fixing lint issues in plugins...$(NC)"
	@for dir in $(PLUGIN_DIRS); do \
		echo "$(YELLOW)Fixing $$dir...$(NC)"; \
		(cd plugins/$$dir && npm run lint:fix) || exit 1; \
	done
	@echo "$(GREEN)All plugin lint issues fixed!$(NC)"

plugins-test: ## Run tests on all plugins
	@echo "$(BLUE)Testing plugins...$(NC)"
	@for dir in $(PLUGIN_DIRS); do \
		echo "$(YELLOW)Testing $$dir...$(NC)"; \
		(cd plugins/$$dir && npm run test) || exit 1; \
	done
	@echo "$(GREEN)All plugin tests passed!$(NC)"

plugins-typecheck: ## Run typecheck on all plugins
	@echo "$(BLUE)Typechecking plugins...$(NC)"
	@cd plugins/sdk-typescript && npm run build
	@for dir in $(PLUGIN_DIRS); do \
		echo "$(YELLOW)Typechecking $$dir...$(NC)"; \
		(cd plugins/$$dir && npx tsc --noEmit) || exit 1; \
	done
	@echo "$(GREEN)All plugins typechecked!$(NC)"

plugins-check: ## Run lint, typecheck, and tests on all plugins
	@$(MAKE) plugins-lint
	@$(MAKE) plugins-typecheck
	@$(MAKE) plugins-test
	@echo "$(GREEN)All plugin checks passed!$(NC)"

plugins-check-fix: ## Run lint:fix, typecheck, and tests on all plugins
	@$(MAKE) plugins-lint-fix
	@$(MAKE) plugins-typecheck
	@$(MAKE) plugins-test
	@echo "$(GREEN)All plugin checks passed!$(NC)"

plugins-clean: ## Clean build artifacts from all plugins
	@echo "$(BLUE)Cleaning plugins...$(NC)"
	@for dir in $(PLUGIN_DIRS); do \
		echo "$(YELLOW)Cleaning $$dir...$(NC)"; \
		(cd plugins/$$dir && npm run clean) || exit 1; \
	done
	@echo "$(GREEN)All plugins cleaned!$(NC)"

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

docker-build-clean-cache: ## Clean build cache
	docker buildx prune --filter type=exec.cachemount -f

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
# Screenshot Automation (Playwright)
# =============================================================================
# Automated screenshot capture for documentation and marketing.
# Requires fixture files in screenshots/fixtures/{comics,manga,books}/
#
# Quick start:
#   1. Add sample files to screenshots/fixtures/
#   2. Run: make screenshots
#   3. Find screenshots in screenshots/output/

screenshots: ## Run full screenshot workflow (start, capture, stop)
	@echo "$(BLUE)Building plugins...$(NC)"
	@$(MAKE) plugins-build
	@echo "$(BLUE)Starting screenshot automation...$(NC)"
	@$(MAKE) screenshots-up
	@echo "$(YELLOW)Waiting for services to be ready...$(NC)"
	@sleep 10
	@$(MAKE) screenshots-run || ($(MAKE) screenshots-down && exit 1)
	@$(MAKE) screenshots-down
	@echo "$(GREEN)Screenshots complete! Check screenshots/output/$(NC)"

screenshots-fresh: ## Run full screenshot workflow with fresh plugins
	@$(MAKE) screenshots-clean
	@$(MAKE) screenshots-down
	@$(MAKE) screenshots

screenshots-up: ## Start screenshot environment
	docker compose --profile screenshots up -d --build
	@echo "$(GREEN)Screenshot environment started$(NC)"

screenshots-down: ## Stop screenshot environment
	docker compose --profile screenshots down
	@echo "$(GREEN)Screenshot environment stopped$(NC)"

screenshots-down-v: ## Stop screenshot environment and remove volumes
	docker compose --profile screenshots down -v
	@echo "$(GREEN)Screenshot environment stopped and volumes removed$(NC)"

screenshots-run: ## Run screenshot capture (requires environment running)
	@echo "$(YELLOW)Running screenshot capture...$(NC)"
	docker compose --profile screenshots exec playwright npm run capture

screenshots-logs: ## View screenshot environment logs
	docker compose --profile screenshots logs -f

screenshots-shell: ## Open shell in Playwright container
	docker compose --profile screenshots exec playwright sh

screenshots-clean: ## Remove generated screenshots
	rm -rf screenshots/output/*
	@echo "$(GREEN)Screenshots cleaned$(NC)"

screenshots-move-to-docs: ## Move screenshots to docs/screenshots
	mkdir -p docs/docs/screenshots
	cp -r screenshots/output/* docs/screenshots/
	@echo "$(GREEN)Screenshots moved to docs/screenshots$(NC)"

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
	git-cliff --unreleased --prepend CHANGELOG.md

changelog-unreleased: ## Show unreleased changes (preview)
	git-cliff --unreleased

changelog-release: ## Generate changelog for a new release (usage: make changelog-release VERSION=1.0.0)
	@if [ -z "$(VERSION)" ]; then \
		echo "$(YELLOW)Error: VERSION not set. Use: make changelog-release VERSION=1.0.0$(NC)"; \
		exit 1; \
	fi
	git-cliff --unreleased --tag v$(VERSION) --prepend CHANGELOG.md
	@echo "$(GREEN)Changelog generated for v$(VERSION)$(NC)"

# =============================================================================
# Release
# =============================================================================

release-prepare: ## Prepare a release (usage: make release-prepare VERSION=1.0.0)
	@if [ -z "$(VERSION)" ]; then \
		echo "$(YELLOW)Error: VERSION not set. Use: make release-prepare VERSION=1.0.0$(NC)"; \
		exit 1; \
	fi
	@echo "$(BLUE)Preparing release v$(VERSION)...$(NC)"
	@echo ""
	@# Update Cargo.toml version
	@sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml && rm Cargo.toml.bak
	@echo "$(GREEN)✓$(NC) Cargo.toml version set to $(VERSION)"

	@# Update web/package.json version
	@cd web && npm version $(VERSION) --no-git-tag-version --allow-same-version >/dev/null 2>&1
	@echo "$(GREEN)✓$(NC) web/package.json version set to $(VERSION)"

	@# Update plugins/sdk-typescript/package.json version
	@cd plugins/sdk-typescript && npm version $(VERSION) --no-git-tag-version --allow-same-version >/dev/null 2>&1
	@echo "$(GREEN)✓$(NC) plugins/sdk-typescript/package.json version set to $(VERSION)"

	@# Update plugins/metadata-echo/package.json version
	@cd plugins/metadata-echo && npm version $(VERSION) --no-git-tag-version --allow-same-version >/dev/null 2>&1
	@echo "$(GREEN)✓$(NC) plugins/metadata-echo/package.json version set to $(VERSION)"

	@# Update plugins/metadata-mangabaka/package.json version
	@cd plugins/metadata-mangabaka && npm version $(VERSION) --no-git-tag-version --allow-same-version >/dev/null 2>&1
	@echo "$(GREEN)✓$(NC) plugins/metadata-mangabaka/package.json version set to $(VERSION)"

	@# Update docs/package.json version
	@cd docs && npm version $(VERSION) --no-git-tag-version --allow-same-version >/dev/null 2>&1
	@echo "$(GREEN)✓$(NC) docs/package.json version set to $(VERSION)"

	@# Update Cargo.lock
	@cargo build --quiet 2>/dev/null || cargo build
	@echo "$(GREEN)✓$(NC) Updated Cargo.lock"

	@# Generate changelog (skip if already modified)
	@if git diff --quiet CHANGELOG.md 2>/dev/null && git diff --cached --quiet CHANGELOG.md 2>/dev/null; then \
		$(MAKE) changelog-release VERSION=$(VERSION); \
		echo "$(GREEN)✓$(NC) Generated CHANGELOG.md for v$(VERSION)"; \
	else \
		echo "$(YELLOW)⊘$(NC) Skipped CHANGELOG.md (already modified)"; \
		echo "   To regenerate: git checkout CHANGELOG.md && make changelog-release VERSION=$(VERSION)"; \
	fi
	@echo ""
	@echo "$(BLUE)═══════════════════════════════════════════════════════════════$(NC)"
	@echo "$(GREEN)Release v$(VERSION) prepared!$(NC)"
	@echo "$(BLUE)═══════════════════════════════════════════════════════════════$(NC)"
	@echo ""
	@echo "$(YELLOW)Next steps:$(NC)"
	@echo "  1. Review the changes:"
	@echo "     $(GREEN)git diff$(NC)"
	@echo ""
	@echo "  2. Commit the release:"
	@echo "     $(GREEN)git add -A && git commit -m \"chore(release): v$(VERSION)\"$(NC)"
	@echo ""
	@echo "  3. Create the tag:"
	@echo "     $(GREEN)git tag -a v$(VERSION) -m \"v$(VERSION)\"$(NC)"
	@echo ""
	@echo "  4. Push to remote:"
	@echo "     $(GREEN)git push && git push --tags$(NC)"
	@echo ""
