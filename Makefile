.PHONY: analyze rerun build-renderer build-collector

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
		--style network \
		--report-output "analysis/$(SLUG)/report.png"

build-renderer:
	cd renderer && cargo build --release

build-collector:
	cd collector && uv sync
