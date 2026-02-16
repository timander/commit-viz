#!/usr/bin/env bash
# commit-viz environment health check
# Verifies all required and optional tools are installed with compatible versions.
set -euo pipefail

PASS=0
WARN=0
FAIL=0

pass()  { PASS=$((PASS + 1)); printf "  \033[32m✓\033[0m %s\n" "$1"; }
warn()  { WARN=$((WARN + 1)); printf "  \033[33m⚠\033[0m %s\n" "$1"; }
fail()  { FAIL=$((FAIL + 1)); printf "  \033[31m✗\033[0m %s\n" "$1"; }
info()  { printf "  \033[90m  %s\033[0m\n" "$1"; }
header(){ printf "\n\033[1m%s\033[0m\n" "$1"; }

header "commit-viz doctor"
echo

# ── Required tools ───────────────────────────────────────────────────────────

header "Required"

# git
if command -v git &>/dev/null; then
    GIT_VERSION=$(git --version | awk '{print $3}')
    pass "git $GIT_VERSION"
else
    fail "git not found — install from https://git-scm.com"
fi

# python
if command -v python3 &>/dev/null; then
    PY_VERSION=$(python3 --version 2>&1 | awk '{print $2}')
    PY_MAJOR=$(echo "$PY_VERSION" | cut -d. -f1)
    PY_MINOR=$(echo "$PY_VERSION" | cut -d. -f2)
    if [ "$PY_MAJOR" -ge 3 ] && [ "$PY_MINOR" -ge 11 ]; then
        pass "python $PY_VERSION (>= 3.11 required)"
    else
        fail "python $PY_VERSION found but >= 3.11 required"
    fi
else
    fail "python3 not found — install Python 3.11+"
fi

# uv
if command -v uv &>/dev/null; then
    UV_VERSION=$(uv --version 2>&1 | awk '{print $2}')
    pass "uv $UV_VERSION"
else
    fail "uv not found — install from https://docs.astral.sh/uv/"
    info "curl -LsSf https://astral.sh/uv/install.sh | sh"
fi

# rust / cargo
if command -v cargo &>/dev/null; then
    CARGO_VERSION=$(cargo --version | awk '{print $2}')
    pass "cargo $CARGO_VERSION"
else
    fail "cargo not found — install from https://rustup.rs"
    info "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

if command -v rustc &>/dev/null; then
    RUSTC_VERSION=$(rustc --version | awk '{print $2}')
    pass "rustc $RUSTC_VERSION"
else
    fail "rustc not found — install via rustup"
fi

# ffmpeg
if command -v ffmpeg &>/dev/null; then
    FFMPEG_VERSION=$(ffmpeg -version 2>&1 | head -1 | awk '{print $3}')
    pass "ffmpeg $FFMPEG_VERSION"
else
    fail "ffmpeg not found — required for video rendering"
    info "brew install ffmpeg  (macOS) or apt install ffmpeg (Linux)"
fi

# ── Optional tools ───────────────────────────────────────────────────────────

header "Optional"

# gh cli
if command -v gh &>/dev/null; then
    GH_VERSION=$(gh --version 2>&1 | head -1 | awk '{print $3}')
    # Check if authenticated
    if gh auth status &>/dev/null 2>&1; then
        GH_USER=$(gh api user --jq .login 2>/dev/null || echo "unknown")
        pass "gh $GH_VERSION (authenticated as $GH_USER)"
    else
        warn "gh $GH_VERSION (not authenticated — run 'gh auth login')"
    fi
else
    warn "gh cli not found — needed for GitHub API features"
    info "brew install gh  (macOS) or https://cli.github.com"
fi

# ssh (for git@ URLs)
if command -v ssh &>/dev/null; then
    # Test if SSH agent has keys loaded
    SSH_KEYS=$(ssh-add -l 2>/dev/null | grep -c "SHA256" || true)
    if [ "$SSH_KEYS" -gt 0 ]; then
        pass "ssh agent ($SSH_KEYS key(s) loaded)"
    else
        warn "ssh available but no keys in agent — git@ URLs may fail"
        info "ssh-add ~/.ssh/id_ed25519  (or your key path)"
    fi
else
    warn "ssh not found — git@ clone URLs won't work"
fi

# ── Environment variables ────────────────────────────────────────────────────

header "Environment"

# .env file
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [ -f "$PROJECT_ROOT/.env" ]; then
    pass ".env file found"
else
    info "No .env file (optional — secrets can be set as env vars)"
fi

# JIRA
if [ -n "${JIRA_API_TOKEN:-}" ]; then
    pass "JIRA_API_TOKEN is set"
else
    info "JIRA_API_TOKEN not set (needed for Jira integration)"
fi

if [ -n "${JIRA_USER_EMAIL:-}" ]; then
    pass "JIRA_USER_EMAIL is set"
else
    info "JIRA_USER_EMAIL not set (needed for Jira integration)"
fi

# GITHUB_TOKEN
if [ -n "${GITHUB_TOKEN:-}" ]; then
    pass "GITHUB_TOKEN is set"
else
    # Check if gh cli can provide a token
    if command -v gh &>/dev/null && gh auth status &>/dev/null 2>&1; then
        info "GITHUB_TOKEN not set but gh cli is authenticated (can use 'gh api')"
    else
        info "GITHUB_TOKEN not set (optional — for GitHub API rate limits)"
    fi
fi

# ── Project state ────────────────────────────────────────────────────────────

header "Project"

# Collector dependencies
if [ -f "$PROJECT_ROOT/collector/pyproject.toml" ]; then
    if [ -d "$PROJECT_ROOT/collector/.venv" ]; then
        pass "Collector venv exists"
    else
        warn "Collector venv not found — run 'cd collector && uv sync'"
    fi
else
    fail "collector/pyproject.toml not found"
fi

# Renderer binary
if [ -f "$PROJECT_ROOT/renderer/target/release/commit-viz-renderer" ]; then
    pass "Renderer binary built"
else
    warn "Renderer not built — run 'make build-renderer'"
fi

# Existing analyses
ANALYSIS_COUNT=0
if [ -d "$PROJECT_ROOT/analysis" ]; then
    for dir in "$PROJECT_ROOT/analysis"/*/; do
        [ -f "$dir/config.yaml" ] 2>/dev/null && ANALYSIS_COUNT=$((ANALYSIS_COUNT + 1))
    done
fi
if [ "$ANALYSIS_COUNT" -gt 0 ]; then
    info "$ANALYSIS_COUNT analysis project(s) found"
else
    info "No analysis projects yet — run 'make analyze' to start"
fi

# ── Summary ──────────────────────────────────────────────────────────────────

echo
header "Summary"
printf "  %d passed, %d warnings, %d failed\n" "$PASS" "$WARN" "$FAIL"

if [ "$FAIL" -gt 0 ]; then
    echo
    printf "  \033[31mFix the failures above before running commit-viz.\033[0m\n"
    exit 1
elif [ "$WARN" -gt 0 ]; then
    echo
    printf "  \033[33mAll required tools present. Warnings are for optional features.\033[0m\n"
else
    echo
    printf "  \033[32mAll checks passed. Ready to go.\033[0m\n"
fi
