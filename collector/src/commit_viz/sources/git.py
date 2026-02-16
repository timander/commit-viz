from __future__ import annotations

import os
import re
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone

from git import Repo

from commit_viz.config import Config
from commit_viz.models import Branch, Commit, Merge

CONVENTIONAL_RE = re.compile(
    r"^(?P<type>feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)"
    r"(?:\(.*?\))?!?:\s"
)
TICKET_RE = re.compile(r"(?P<ticket>[A-Z][A-Z0-9]+-\d+)")

# Keywords for category classification when conventional type is not present.
# Order matters: first match wins. More specific patterns before general ones.
_CATEGORY_KEYWORDS: list[tuple[str, list[str]]] = [
    ("merge", [
        "merge pull request", "merge branch", "merge remote",
        "merged in", "merge commit", "squash and merge",
    ]),
    ("squash", [
        "squash", "squashed commit", "fixup!", "amend",
    ]),
    ("conflict", [
        "merge conflict", "conflict resolution", "resolve conflict",
        "fix conflict", "resolved merge", "fix merge",
    ]),
    ("release", ["release", "bump version", "version bump", "prepare release"]),
    ("bugfix", ["fix", "bugfix", "hotfix", "patch", "repair", "resolve"]),
    ("feature", ["feat", "add", "implement", "introduce", "new"]),
    ("docs", ["doc", "readme", "changelog", "license", "comment"]),
    ("test", ["test", "spec", "coverage"]),
    ("ci", ["ci", "pipeline", "workflow", "github action", "travis", "jenkins"]),
    ("refactor", ["refactor", "restructure", "reorganize", "clean", "simplify"]),
]

# Map conventional commit types to categories
_CONVENTIONAL_TO_CATEGORY: dict[str, str] = {
    "feat": "feature",
    "fix": "bugfix",
    "docs": "docs",
    "style": "refactor",
    "refactor": "refactor",
    "perf": "refactor",
    "test": "test",
    "build": "ci",
    "ci": "ci",
    "chore": "other",
    "revert": "other",
}


def _progress(msg: str, end: str = "") -> None:
    """Write a progress message to stderr, overwriting the current line."""
    sys.stderr.write(f"\r  {msg}".ljust(80) + end)
    sys.stderr.flush()


def _parse_conventional_type(message: str) -> str | None:
    m = CONVENTIONAL_RE.match(message)
    return m.group("type") if m else None


def _parse_ticket_id(message: str) -> str | None:
    m = TICKET_RE.search(message)
    return m.group("ticket") if m else None


def _classify_category(
    message: str,
    conventional_type: str | None,
    is_merge_commit: bool,
) -> str:
    """Classify a commit into a category.

    Priority:
    1. Merge/squash/conflict detection (structural, from message keywords)
    2. Conventional commit type prefix
    3. Keyword-based fallback
    """
    msg_lower = message.lower()

    # Check merge/squash/conflict first — these override conventional types
    # because a "fix: resolve merge conflict" is really about the conflict
    for category in ("conflict", "merge", "squash"):
        for cat, keywords in _CATEGORY_KEYWORDS:
            if cat != category:
                continue
            for kw in keywords:
                if kw in msg_lower:
                    return category

    # Auto-detect merge commits by parent count even if message doesn't say so
    if is_merge_commit and not any(
        kw in msg_lower
        for kw in ("feat", "fix", "add", "implement", "release", "doc", "test")
    ):
        return "merge"

    # Conventional commit type
    if conventional_type:
        return _CONVENTIONAL_TO_CATEGORY.get(conventional_type, "other")

    # Keyword-based fallback (skip merge/squash/conflict — already checked above)
    for category, keywords in _CATEGORY_KEYWORDS:
        if category in ("merge", "squash", "conflict"):
            continue
        for kw in keywords:
            if kw in msg_lower:
                return category

    return "other"


def _parse_date_bound(value: str) -> datetime | None:
    """Parse a date-range bound, returning None for empty/special values like 'all'."""
    if not value or value.lower() in ("all", "beginning", "present", "now", "today"):
        return None
    try:
        return datetime.fromisoformat(value).replace(tzinfo=timezone.utc)
    except ValueError:
        return None


def _in_date_range(ts: datetime, start: str, end: str) -> bool:
    start_dt = _parse_date_bound(start)
    if start_dt and ts < start_dt:
        return False
    end_dt = _parse_date_bound(end)
    if end_dt and ts > end_dt:
        return False
    return True


def _commit_timestamp(commit) -> datetime:
    return commit.committed_datetime.astimezone(timezone.utc)


def _collect_numstat(repo_path: str) -> dict[str, tuple[int, int, int]]:
    """Run git log --numstat with streaming output and stall detection.

    Returns a dict mapping SHA -> (insertions, deletions, files_changed).
    """
    _progress("Running git log --numstat (streaming)...")

    t0 = time.monotonic()

    # Use Popen for streaming instead of run() to avoid buffering the entire output
    proc = subprocess.Popen(
        ["git", "log", "--all", "--numstat", "--format=%H"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        cwd=repo_path,
    )

    stats: dict[str, tuple[int, int, int]] = {}
    current_sha: str | None = None
    ins = 0
    dels = 0
    files = 0
    line_count = 0
    last_progress_time = time.monotonic()

    # Watchdog: print elapsed time if no progress update for 10 seconds.
    # This runs in a background thread so the user knows the process is alive.
    stop_watchdog = threading.Event()

    def watchdog():
        while not stop_watchdog.is_set():
            stop_watchdog.wait(10.0)
            if stop_watchdog.is_set():
                break
            elapsed = time.monotonic() - t0
            _progress(
                f"Numstat: {len(stats)} commits, {line_count} lines read "
                f"[{elapsed:.0f}s elapsed, still running...]"
            )

    watcher = threading.Thread(target=watchdog, daemon=True)
    watcher.start()

    for line in proc.stdout:
        line_count += 1
        line = line.strip()
        if not line:
            continue

        # A 40-char hex string is a commit SHA
        if len(line) == 40 and all(c in "0123456789abcdef" for c in line):
            if current_sha is not None:
                stats[current_sha] = (ins, dels, files)
            current_sha = line
            ins = 0
            dels = 0
            files = 0

            now = time.monotonic()
            if len(stats) % 200 == 0 and now - last_progress_time > 0.5:
                elapsed = now - t0
                _progress(
                    f"Numstat: {len(stats)} commits parsed, "
                    f"{line_count} lines [{elapsed:.0f}s]..."
                )
                last_progress_time = now
        elif current_sha is not None:
            parts = line.split("\t")
            if len(parts) >= 3:
                try:
                    i = int(parts[0]) if parts[0] != "-" else 0
                    d = int(parts[1]) if parts[1] != "-" else 0
                    ins += i
                    dels += d
                    files += 1
                except ValueError:
                    pass

    # Save last commit
    if current_sha is not None:
        stats[current_sha] = (ins, dels, files)

    proc.wait()
    stop_watchdog.set()
    watcher.join(timeout=1.0)

    elapsed = time.monotonic() - t0
    _progress(f"Numstat: {len(stats)} commits, {line_count} lines [{elapsed:.1f}s]", end="\n")

    return stats


def _build_branch_membership(
    repo, refs: list, max_workers: int,
) -> dict[str, set[str]]:
    """Build branch membership map: commit SHA -> set of branch names.

    Parallelizes across refs using ThreadPoolExecutor (GitPython subprocess
    calls release the GIL).
    """
    _progress("Building branch membership map (parallel)...")
    commit_to_branches: dict[str, set[str]] = {}
    lock = threading.Lock()
    total_refs = len(refs)
    ref_count = 0
    t0 = time.monotonic()
    last_progress_time = time.monotonic()

    def process_ref(ref):
        nonlocal ref_count, last_progress_time
        branch_name = ref.name
        if branch_name.startswith("origin/"):
            branch_name = branch_name[len("origin/"):]
        if branch_name == "HEAD":
            return

        local_map: dict[str, str] = {}
        for c in repo.iter_commits(ref):
            local_map[c.hexsha] = branch_name

        with lock:
            for sha, bname in local_map.items():
                commit_to_branches.setdefault(sha, set()).add(bname)
            ref_count += 1
            now = time.monotonic()
            if (ref_count % 10 == 0 or ref_count == total_refs) and now - last_progress_time > 0.3:
                elapsed = now - t0
                _progress(
                    f"Branch map: {ref_count}/{total_refs} refs, "
                    f"{len(commit_to_branches)} commits [{elapsed:.0f}s]..."
                )
                last_progress_time = now

    with ThreadPoolExecutor(max_workers=max_workers) as pool:
        list(pool.map(process_ref, refs))

    elapsed = time.monotonic() - t0
    _progress(
        f"Branch map: {len(commit_to_branches)} commits across {total_refs} refs [{elapsed:.1f}s]",
        end="\n",
    )
    return commit_to_branches


def _detect_default_branch(repo, branch_names: set[str]) -> str:
    """Detect default branch with priority: main > active_branch > master > sorted first."""
    if "main" in branch_names:
        return "main"
    try:
        active = repo.active_branch.name
        if active in branch_names:
            return active
    except TypeError:
        pass
    if "master" in branch_names:
        return "master"
    return sorted(branch_names)[0] if branch_names else "main"


def collect_git(config: Config) -> tuple[list[Branch], list[Commit], list[Merge]]:
    repo_path = config.repo.path
    if repo_path is None:
        raise ValueError("repo.path is required for git collection")

    _progress("Opening repository...")
    repo = Repo(repo_path)

    # Collect branches
    _progress("Scanning branches...")
    branch_names: set[str] = set()
    for ref in repo.references:
        name = ref.name
        if name.startswith("origin/"):
            name = name[len("origin/"):]
        if name == "HEAD":
            continue
        branch_names.add(name)

    # Determine default branch (prefer "main" if it exists)
    default_branch = _detect_default_branch(repo, branch_names)

    branches = [
        Branch(name=name, is_default=(name == default_branch))
        for name in sorted(branch_names)
    ]
    _progress(f"Found {len(branches)} branches", end="\n")

    # Build tag map: commit sha -> list of tag names
    _progress("Scanning tags...")
    tag_map: dict[str, list[str]] = {}
    for tag in repo.tags:
        sha = tag.commit.hexsha
        tag_map.setdefault(sha, []).append(tag.name)
    _progress(f"Found {len(tag_map)} tagged commits", end="\n")

    # Run branch membership and numstat concurrently
    # (GitPython subprocess calls release the GIL)
    refs = list(repo.references)
    cpu_count = os.cpu_count() or 1
    with ThreadPoolExecutor(max_workers=2) as pool:
        future_membership = pool.submit(_build_branch_membership, repo, refs, cpu_count)
        future_numstat = pool.submit(_collect_numstat, repo_path)
        commit_to_branches = future_membership.result()
        numstat = future_numstat.result()

    # Walk all commits
    _progress("Processing commits...")
    seen: set[str] = set()
    commits: list[Commit] = []
    merges: list[Merge] = []

    start = config.date_range.start
    end = config.date_range.end

    t0 = time.monotonic()
    ref_count = 0
    last_progress_time = time.monotonic()

    for ref in repo.references:
        ref_count += 1
        for c in repo.iter_commits(ref):
            if c.hexsha in seen:
                continue
            seen.add(c.hexsha)

            ts = _commit_timestamp(c)
            if not _in_date_range(ts, start, end):
                continue

            # Pick the most specific branch for this commit
            candidate_branches = commit_to_branches.get(c.hexsha, {default_branch})
            non_default = candidate_branches - {default_branch}
            branch = sorted(non_default)[0] if non_default else default_branch

            conv_type = _parse_conventional_type(c.message)
            message_line = c.message.strip().split("\n")[0]

            is_merge = len(c.parents) >= 2
            category = _classify_category(message_line, conv_type, is_merge)

            # Detect squash commits: single parent but message indicates bundling
            is_squash = (
                not is_merge
                and any(
                    kw in message_line.lower()
                    for kw in ("squash", "squashed", "fixup!")
                )
            )

            # Get numstat data
            ins, dels, fchanged = numstat.get(c.hexsha, (0, 0, 0))

            commit = Commit(
                sha=c.hexsha,
                author=c.author.name or "",
                timestamp=ts.isoformat(),
                branch=branch,
                message=message_line,
                parents=[p.hexsha for p in c.parents],
                tags=tag_map.get(c.hexsha, []),
                conventional_type=conv_type,
                ticket_id=_parse_ticket_id(c.message),
                insertions=ins,
                deletions=dels,
                files_changed=fchanged,
                category=category,
                is_merge_commit=is_merge,
                is_squash=is_squash,
            )
            commits.append(commit)

            # Detect merge commits (2+ parents)
            if is_merge:
                merge = Merge(
                    sha=c.hexsha,
                    from_branch=branch if branch != default_branch else "unknown",
                    to_branch=default_branch,
                    timestamp=ts.isoformat(),
                )
                merges.append(merge)

            now = time.monotonic()
            if len(seen) % 500 == 0 and now - last_progress_time > 0.3:
                elapsed = now - t0
                _progress(
                    f"Commits: {len(seen)} seen, {len(commits)} matched, "
                    f"ref {ref_count}/{total_refs} [{elapsed:.0f}s]..."
                )
                last_progress_time = now

    elapsed = time.monotonic() - t0
    _progress(
        f"Commits: {len(seen)} seen, {len(commits)} in range, {len(merges)} merges [{elapsed:.1f}s]",
        end="\n",
    )

    # Sort commits by timestamp
    commits.sort(key=lambda c: c.timestamp)
    merges.sort(key=lambda m: m.timestamp)

    return branches, commits, merges
