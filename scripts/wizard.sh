#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ANALYSIS_DIR="$PROJECT_ROOT/analysis"

echo "=== commit-viz wizard ==="
echo

# Detect existing projects
EXISTING_PROJECTS=()
if [ -d "$ANALYSIS_DIR" ]; then
    for dir in "$ANALYSIS_DIR"/*/; do
        [ -d "$dir" ] || continue
        slug=$(basename "$dir")
        [ -f "$dir/config.yaml" ] || continue
        EXISTING_PROJECTS+=("$slug")
    done
fi

ACTION="new"
SLUG=""

if [ ${#EXISTING_PROJECTS[@]} -gt 0 ]; then
    echo "Existing projects:"
    for i in "${!EXISTING_PROJECTS[@]}"; do
        slug="${EXISTING_PROJECTS[$i]}"
        dir="$ANALYSIS_DIR/$slug"
        # Show summary info
        has_video=""
        has_data=""
        has_charts=""
        [ -f "$dir/$slug.mp4" ] && has_video="video"
        [ -f "$dir/output.json" ] && has_data="data"
        [ -d "$dir/change-flow" ] && has_charts="charts"
        artifacts=$(echo "$has_video $has_data $has_charts" | xargs)
        echo "  $((i+1))) $slug  [$artifacts]"
    done
    echo "  N) Analyze a new repository"
    echo
    read -rp "Select a project to rerun, or N for new [N]: " CHOICE
    CHOICE="${CHOICE:-N}"

    if [[ "$CHOICE" =~ ^[0-9]+$ ]] && [ "$CHOICE" -ge 1 ] && [ "$CHOICE" -le ${#EXISTING_PROJECTS[@]} ]; then
        SLUG="${EXISTING_PROJECTS[$((CHOICE-1))]}"
        ACTION="rerun"
        echo
        echo "  Rerunning: $SLUG"
    else
        ACTION="new"
    fi
fi

if [ "$ACTION" = "new" ]; then
    echo
    # 1. Repository URL or local path
    read -rp "Repository (GitHub URL or local path): " REPO_INPUT

    # Determine slug and whether to clone
    if [[ "$REPO_INPUT" =~ ^https?:// ]]; then
        SLUG=$(basename "$REPO_INPUT" .git)
        REPO_URL="$REPO_INPUT"
        REPO_PATH_CONFIG="repo"
    else
        SLUG=$(basename "$REPO_INPUT")
        REPO_URL=""
        REPO_PATH_CONFIG="$REPO_INPUT"
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
else
    ANALYSIS_SLUG_DIR="$ANALYSIS_DIR/$SLUG"
    CONFIG_PATH="$ANALYSIS_SLUG_DIR/config.yaml"
fi

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

# Read speed mode from config for duration flag
DURATION_FLAG=""
if grep -q "mode: duration" "$CONFIG_PATH" 2>/dev/null; then
    SPEED_VALUE=$(grep "value:" "$CONFIG_PATH" | tail -1 | awk '{print $2}')
    DURATION_FLAG="--duration-secs ${SPEED_VALUE%.*}"
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
echo "  Charts: $ANALYSIS_SLUG_DIR/change-flow/"
echo "  Data:  $OUTPUT_JSON"
