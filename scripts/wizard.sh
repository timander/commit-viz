#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ANALYSIS_DIR="$PROJECT_ROOT/analysis"

echo "=== commit-viz wizard ==="
echo

# 1. Repository URL or local path
read -rp "Repository (GitHub URL or local path): " REPO_INPUT

# Determine slug and whether to clone
if [[ "$REPO_INPUT" =~ ^https?:// ]]; then
    # Extract slug from URL (last path component, without .git)
    SLUG=$(basename "$REPO_INPUT" .git)
    REPO_URL="$REPO_INPUT"
    REPO_PATH_CONFIG="repo"  # relative to config file location
    REPO_PATH_ABS="$ANALYSIS_DIR/$SLUG/repo"  # for runtime use
else
    # Local path — use directory name as slug
    SLUG=$(basename "$REPO_INPUT")
    REPO_URL=""
    REPO_PATH_CONFIG="$REPO_INPUT"  # keep user-provided path
    REPO_PATH_ABS="$(cd "$REPO_INPUT" 2>/dev/null && pwd || echo "$REPO_INPUT")"
fi

echo "  Project slug: $SLUG"

# 2. Date range
read -rp "Date range start [all]: " DATE_START
DATE_START="${DATE_START:-}"

read -rp "Date range end [today]: " DATE_END
DATE_END="${DATE_END:-}"

# 3. Video pace
echo "Video pace:"
echo "  1) per day"
echo "  2) per week"
echo "  3) per month"
echo "  4) fixed duration (seconds)"
read -rp "Choice [3]: " PACE_CHOICE
PACE_CHOICE="${PACE_CHOICE:-3}"

case "$PACE_CHOICE" in
    1) SPEED_MODE="per_day";  SPEED_VALUE="1.0" ;;
    2) SPEED_MODE="per_week"; SPEED_VALUE="1.0" ;;
    3) SPEED_MODE="per_month"; SPEED_VALUE="1.0" ;;
    4)
        SPEED_MODE="duration"
        read -rp "Total duration in seconds [30]: " SPEED_VALUE
        SPEED_VALUE="${SPEED_VALUE:-30}"
        ;;
    *) SPEED_MODE="per_month"; SPEED_VALUE="1.0" ;;
esac

# Create analysis directory
ANALYSIS_SLUG_DIR="$ANALYSIS_DIR/$SLUG"
mkdir -p "$ANALYSIS_SLUG_DIR"

# Generate config.yaml
CONFIG_PATH="$ANALYSIS_SLUG_DIR/config.yaml"
cat > "$CONFIG_PATH" <<EOF
repo:
  path: $REPO_PATH_CONFIG
  url: $REPO_URL

date_range:
  start: "$DATE_START"
  end: "$DATE_END"

sources:
  git: true
  github_actions: false
  jira:
    enabled: false
    projects: []
    base_url: ""

rendering:
  style: network
  output: $SLUG.mp4
  fps: 30
  resolution: [1920, 1080]
  video_speed:
    mode: $SPEED_MODE
    value: $SPEED_VALUE
EOF

echo
echo "Config written to $CONFIG_PATH"
echo

# Run collector
OUTPUT_JSON="$ANALYSIS_SLUG_DIR/output.json"
echo "Running collector..."
cd "$PROJECT_ROOT/collector"
uv run commit-viz collect --config "$CONFIG_PATH" --output "$OUTPUT_JSON"
echo

# Build renderer if needed
echo "Building renderer..."
cd "$PROJECT_ROOT/renderer"
cargo build --release 2>&1 | tail -1
RENDERER="$PROJECT_ROOT/renderer/target/release/commit-viz-renderer"

# Compute duration from speed config
DURATION_FLAG=""
if [ "$SPEED_MODE" = "duration" ]; then
    DURATION_FLAG="--duration-secs $SPEED_VALUE"
fi

# Run renderer
echo "Running renderer..."
$RENDERER --input "$OUTPUT_JSON" --output "$ANALYSIS_SLUG_DIR/$SLUG.mp4" \
    --style network \
    --report-output "$ANALYSIS_SLUG_DIR/report.png" \
    --change-flow-dir "$ANALYSIS_SLUG_DIR/change-flow" \
    $DURATION_FLAG

echo
echo "=== Done! ==="
echo "  Video: $ANALYSIS_SLUG_DIR/$SLUG.mp4"
echo "  Report: $ANALYSIS_SLUG_DIR/report.png"
echo "  Data:  $OUTPUT_JSON"
