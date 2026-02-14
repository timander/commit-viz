from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class Branch:
    name: str
    is_default: bool = False


@dataclass
class Commit:
    sha: str
    author: str
    timestamp: str
    branch: str
    message: str
    parents: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    conventional_type: str | None = None
    ticket_id: str | None = None
    insertions: int = 0
    deletions: int = 0
    files_changed: int = 0
    category: str = "other"


@dataclass
class Merge:
    sha: str
    from_branch: str
    to_branch: str
    timestamp: str


@dataclass
class ReleaseCycleStats:
    count: int = 0
    mean_days: float = 0.0
    min_days: float = 0.0
    max_days: float = 0.0
    stdev_days: float = 0.0


@dataclass
class Statistics:
    total_commits: int = 0
    date_span_days: int = 0
    commits_per_week: float = 0.0
    unique_authors: int = 0
    by_category: dict[str, int] = field(default_factory=dict)
    by_branch: dict[str, int] = field(default_factory=dict)
    top_authors: list[dict[str, int | str]] = field(default_factory=list)
    release_cycles: ReleaseCycleStats = field(default_factory=ReleaseCycleStats)


@dataclass
class Metadata:
    repo: str
    date_range: dict[str, str] = field(default_factory=dict)
    generated_at: str = ""


@dataclass
class CollectedData:
    metadata: Metadata
    branches: list[Branch] = field(default_factory=list)
    commits: list[Commit] = field(default_factory=list)
    merges: list[Merge] = field(default_factory=list)
    deployments: list[dict] = field(default_factory=list)
    ci_runs: list[dict] = field(default_factory=list)
    statistics: Statistics | None = None
