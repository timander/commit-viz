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


@dataclass
class Merge:
    sha: str
    from_branch: str
    to_branch: str
    timestamp: str


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
