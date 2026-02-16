#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ANALYSIS_DIR="$PROJECT_ROOT/analysis"

echo "=== commit-viz wizard ==="
echo

# ── Detect existing projects ────────────────────────────────────────────────

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

# ── Helper: parse git URL (HTTPS, SSH, or local path) ───────────────────────

parse_repo_input() {
    local input="$1"

    if [[ "$input" =~ ^https?:// ]]; then
        # HTTPS URL: https://github.com/org/repo.git
        SLUG=$(basename "$input" .git)
        REPO_URL="$input"
        REPO_PATH_CONFIG="repo"

    elif [[ "$input" =~ ^git@ ]]; then
        # SSH URL: git@github.com:org/repo.git
        # Extract the repo name from after the last / or :
        local path_part="${input#*:}"           # org/repo.git
        SLUG=$(basename "$path_part" .git)      # repo

        # Convert to HTTPS for display/config, keep SSH for cloning
        local host="${input%%:*}"               # git@github.com
        host="${host#git@}"                     # github.com
        REPO_URL="$input"
        REPO_PATH_CONFIG="repo"

    elif [[ "$input" =~ ^ssh:// ]]; then
        # ssh://git@github.com/org/repo.git
        SLUG=$(basename "$input" .git)
        REPO_URL="$input"
        REPO_PATH_CONFIG="repo"

    else
        # Local path
        SLUG=$(basename "$input")
        REPO_URL=""
        REPO_PATH_CONFIG="$input"
    fi
}

# ── New project setup ───────────────────────────────────────────────────────

if [ "$ACTION" = "new" ]; then
    echo
    read -rp "Repository (HTTPS URL, SSH git@... URL, or local path): " REPO_INPUT

    parse_repo_input "$REPO_INPUT"

    echo "  Project slug: $SLUG"

    # Date range
    read -rp "Date range start [all]: " DATE_START
    DATE_START="${DATE_START:-}"

    read -rp "Date range end [today]: " DATE_END
    DATE_END="${DATE_END:-}"

    # Video pace
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

    # ── Jira integration ────────────────────────────────────────────────────

    JIRA_ENABLED="false"
    JIRA_BASE_URL=""
    JIRA_PROJECTS_YAML="[]"

    echo
    read -rp "Enable Jira integration? [y/N]: " JIRA_CHOICE
    if [[ "${JIRA_CHOICE:-N}" =~ ^[Yy] ]]; then
        # Check for required env vars
        if [ -z "${JIRA_API_TOKEN:-}" ]; then
            echo "  JIRA_API_TOKEN not set."
            echo "  Set it in your environment or in a .env file at the project root."
            echo "  Example: export JIRA_API_TOKEN=your-token-here"
            read -rp "  Continue without Jira? [Y/n]: " SKIP_JIRA
            if [[ "${SKIP_JIRA:-Y}" =~ ^[Nn] ]]; then
                echo "  Aborting."
                exit 1
            fi
        else
            if [ -z "${JIRA_USER_EMAIL:-}" ]; then
                read -rp "  Jira user email: " JIRA_USER_EMAIL
                export JIRA_USER_EMAIL
            fi
            read -rp "  Jira base URL (e.g. https://yourorg.atlassian.net): " JIRA_BASE_URL
            read -rp "  Jira project keys (comma-separated, e.g. PROJ,TEAM): " JIRA_PROJECTS_INPUT

            JIRA_ENABLED="true"
            # Convert comma-separated to YAML list
            JIRA_PROJECTS_YAML="[$(echo "$JIRA_PROJECTS_INPUT" | sed 's/,/, /g')]"
            echo "  Jira enabled: $JIRA_BASE_URL ($JIRA_PROJECTS_INPUT)"
        fi
    fi

    # ── GitHub Actions integration ──────────────────────────────────────────

    GH_ACTIONS="false"

    # Auto-detect if gh is authenticated and repo is on GitHub
    if [[ "$REPO_URL" == *github.com* ]] && command -v gh &>/dev/null && gh auth status &>/dev/null 2>&1; then
        echo
        read -rp "Enable GitHub Actions data? (gh cli detected) [y/N]: " GH_CHOICE
        if [[ "${GH_CHOICE:-N}" =~ ^[Yy] ]]; then
            GH_ACTIONS="true"
        fi
    fi

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
  github_actions: $GH_ACTIONS
  jira:
    enabled: $JIRA_ENABLED
    projects: $JIRA_PROJECTS_YAML
    base_url: "$JIRA_BASE_URL"

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

# ── Run collector ───────────────────────────────────────────────────────────

OUTPUT_JSON="$ANALYSIS_SLUG_DIR/output.json"
echo "Running collector..."
cd "$PROJECT_ROOT/collector"
uv run commit-viz collect --config "$CONFIG_PATH" --output "$OUTPUT_JSON"
echo

# ── Build and run renderer ──────────────────────────────────────────────────

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
