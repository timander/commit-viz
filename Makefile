.PHONY: help analyze rerun change-flow build-renderer build-collector doctor \
       lint lint-python lint-rust fmt fmt-python fmt-rust test test-python test-rust coverage

.DEFAULT_GOAL := help

# Show all available commands
help:
	@echo "commit-viz — git collaboration visualizer"
	@echo ""
	@echo "Usage: make <target>"
	@echo ""
	@echo "Setup:"
	@echo "  doctor           Run environment health check (versions, tools, env vars)"
	@echo "  build-collector   Install Python collector dependencies (uv sync)"
	@echo "  build-renderer    Build Rust renderer binary (cargo build --release)"
	@echo ""
	@echo "Quality:"
	@echo "  lint              Run all linters (ruff, mypy, clippy)"
	@echo "  fmt               Auto-format all code (ruff format, cargo fmt)"
	@echo "  test              Run all tests (pytest, cargo test)"
	@echo "  coverage          Run tests with coverage reports"
	@echo ""
	@echo "Analysis:"
	@echo "  analyze           Interactive wizard — new repo or rerun existing"
	@echo "  rerun SLUG=<name> Re-run full pipeline on existing project"
	@echo "  change-flow SLUG=<name>"
	@echo "                    Regenerate change-flow charts only (no video)"
	@echo ""
	@echo "Examples:"
	@echo "  make doctor                  # verify all tools are installed"
	@echo "  make lint                    # check code quality"
	@echo "  make analyze                 # launch interactive wizard"
	@echo "  make rerun SLUG=slf4j        # re-collect and re-render slf4j"
	@echo "  make change-flow SLUG=flask  # regenerate flask charts only"

# Environment health check — run first to verify all dependencies
doctor:
	@bash scripts/doctor.sh

# Interactive wizard — prompts for repo, date range, speed
analyze:
	@bash scripts/wizard.sh

# Re-run an existing analysis (skips wizard, uses existing config)
# Usage: make rerun SLUG=slf4j
rerun:
	@test -n "$(SLUG)" || (echo "Usage: make rerun SLUG=<name>" && exit 1)
	@test -f "analysis/$(SLUG)/config.yaml" || (echo "No config found at analysis/$(SLUG)/config.yaml" && exit 1)
	cd collector && uv run commit-viz collect \
		--config "../analysis/$(SLUG)/config.yaml" \
		--output "../analysis/$(SLUG)/output.json"
	cd renderer && cargo build --release
	renderer/target/release/commit-viz-renderer \
		--input "analysis/$(SLUG)/output.json" \
		--output "analysis/$(SLUG)/$(SLUG).mp4" \
		--report-output "analysis/$(SLUG)/report.png" \
		--change-flow-dir "analysis/$(SLUG)/change-flow"

# Generate only change flow visualizations (no video re-render)
# Usage: make change-flow SLUG=slf4j
change-flow:
	@test -n "$(SLUG)" || (echo "Usage: make change-flow SLUG=<name>" && exit 1)
	@test -f "analysis/$(SLUG)/output.json" || (echo "No output.json found at analysis/$(SLUG)/output.json — run 'make rerun SLUG=$(SLUG)' first" && exit 1)
	cd renderer && cargo build --release
	renderer/target/release/commit-viz-renderer \
		--input "analysis/$(SLUG)/output.json" \
		--output "/dev/null" \
		--change-flow-dir "analysis/$(SLUG)/change-flow"

build-renderer:
	cd renderer && cargo build --release

build-collector:
	cd collector && uv sync

# ── Code quality ──────────────────────────────────────────────────────────────

lint: lint-python lint-rust

lint-python:
	cd collector && uv run ruff check src/ tests/
	cd collector && uv run mypy src/

lint-rust:
	cd renderer && cargo clippy --release -- -D warnings

fmt: fmt-python fmt-rust

fmt-python:
	cd collector && uv run ruff format src/ tests/
	cd collector && uv run ruff check --fix src/ tests/

fmt-rust:
	cd renderer && cargo fmt

test: test-python test-rust

test-python:
	cd collector && uv run pytest

test-rust:
	cd renderer && cargo test

coverage:
	cd collector && uv run pytest --cov=commit_viz --cov-report=term-missing
	cd renderer && cargo test
