from __future__ import annotations

import re
from datetime import datetime, timezone

from git import Repo

from commit_viz.config import Config
from commit_viz.models import Branch, Commit, Merge

CONVENTIONAL_RE = re.compile(
    r"^(?P<type>feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)"
    r"(?:\(.*?\))?!?:\s"
)
TICKET_RE = re.compile(r"(?P<ticket>[A-Z][A-Z0-9]+-\d+)")


def _parse_conventional_type(message: str) -> str | None:
    m = CONVENTIONAL_RE.match(message)
    return m.group("type") if m else None


def _parse_ticket_id(message: str) -> str | None:
    m = TICKET_RE.search(message)
    return m.group("ticket") if m else None


def _in_date_range(ts: datetime, start: str, end: str) -> bool:
    if start:
        start_dt = datetime.fromisoformat(start).replace(tzinfo=timezone.utc)
        if ts < start_dt:
            return False
    if end:
        end_dt = datetime.fromisoformat(end).replace(tzinfo=timezone.utc)
        if ts > end_dt:
            return False
    return True


def _commit_timestamp(commit) -> datetime:
    return commit.committed_datetime.astimezone(timezone.utc)


def collect_git(config: Config) -> tuple[list[Branch], list[Commit], list[Merge]]:
    repo_path = config.repo.path
    if repo_path is None:
        raise ValueError("repo.path is required for git collection")

    repo = Repo(repo_path)

    # Determine default branch
    try:
        default_branch = repo.active_branch.name
    except TypeError:
        default_branch = "main"

    # Collect branches
    branch_names: set[str] = set()
    for ref in repo.references:
        name = ref.name
        if name.startswith("origin/"):
            name = name[len("origin/"):]
        if name == "HEAD":
            continue
        branch_names.add(name)

    branches = [
        Branch(name=name, is_default=(name == default_branch))
        for name in sorted(branch_names)
    ]

    # Build tag map: commit sha -> list of tag names
    tag_map: dict[str, list[str]] = {}
    for tag in repo.tags:
        sha = tag.commit.hexsha
        tag_map.setdefault(sha, []).append(tag.name)

    # Build branch membership: walk each branch and record commits
    commit_to_branches: dict[str, set[str]] = {}
    for ref in repo.references:
        branch_name = ref.name
        if branch_name.startswith("origin/"):
            branch_name = branch_name[len("origin/"):]
        if branch_name == "HEAD":
            continue
        for c in repo.iter_commits(ref):
            commit_to_branches.setdefault(c.hexsha, set()).add(branch_name)

    # Walk all commits
    seen: set[str] = set()
    commits: list[Commit] = []
    merges: list[Merge] = []

    start = config.date_range.start
    end = config.date_range.end

    for ref in repo.references:
        for c in repo.iter_commits(ref):
            if c.hexsha in seen:
                continue
            seen.add(c.hexsha)

            ts = _commit_timestamp(c)
            if not _in_date_range(ts, start, end):
                continue

            # Pick the most specific branch for this commit
            candidate_branches = commit_to_branches.get(c.hexsha, {default_branch})
            # Prefer non-default branches for attribution
            non_default = candidate_branches - {default_branch}
            branch = sorted(non_default)[0] if non_default else default_branch

            commit = Commit(
                sha=c.hexsha,
                author=c.author.name or "",
                timestamp=ts.isoformat(),
                branch=branch,
                message=c.message.strip().split("\n")[0],
                parents=[p.hexsha for p in c.parents],
                tags=tag_map.get(c.hexsha, []),
                conventional_type=_parse_conventional_type(c.message),
                ticket_id=_parse_ticket_id(c.message),
            )
            commits.append(commit)

            # Detect merge commits (2+ parents)
            if len(c.parents) >= 2:
                merge = Merge(
                    sha=c.hexsha,
                    from_branch=branch if branch != default_branch else "unknown",
                    to_branch=default_branch,
                    timestamp=ts.isoformat(),
                )
                merges.append(merge)

    # Sort commits by timestamp
    commits.sort(key=lambda c: c.timestamp)
    merges.sort(key=lambda m: m.timestamp)

    return branches, commits, merges
