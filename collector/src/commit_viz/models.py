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
class CommitToReleaseDayEntry:
    date: str
    avg_days_to_release: float
    commit_count: int
    unreleased_count: int


@dataclass
class BranchLifespan:
    branch: str
    first_commit: str
    last_commit: str
    lifespan_days: float
    merged: bool
    commit_count: int


@dataclass
class DailyVelocity:
    date: str
    count: int
    dominant_category: str


@dataclass
class DroughtPeriod:
    start_date: str
    end_date: str
    duration_days: int


@dataclass
class CommitMergeLatencyEntry:
    sha: str
    commit_date: str
    days_to_merge: float | None
    lines_changed: int
    category: str


@dataclass
class ReleaseInterval:
    tag: str
    date: str
    days_since_previous: float


@dataclass
class HistogramBin:
    label: str
    min_val: float
    max_val: float
    count: int


@dataclass
class WorkDispositionSegment:
    category: str
    merge_speed: str
    lines_changed: int
    commit_count: int


@dataclass
class WorkDisposition:
    fast_merged_lines: int = 0
    slow_merged_lines: int = 0
    unmerged_lines: int = 0
    fast_merged_commits: int = 0
    slow_merged_commits: int = 0
    unmerged_commits: int = 0
    segments: list[WorkDispositionSegment] = field(default_factory=list)


@dataclass
class WasteMetrics:
    commit_to_release_days: list[CommitToReleaseDayEntry] = field(default_factory=list)
    release_median_latency: float = 0.0
    release_p90_latency: float = 0.0
    release_pct_within_7d: float = 0.0
    branch_lifespans: list[BranchLifespan] = field(default_factory=list)
    branch_median_lifespan: float = 0.0
    branch_unmerged_count: int = 0
    branch_longest_days: float = 0.0
    daily_velocity: list[DailyVelocity] = field(default_factory=list)
    rolling_7day_avg: list[dict[str, float | str]] = field(default_factory=list)
    drought_periods: list[DroughtPeriod] = field(default_factory=list)
    drought_count: int = 0
    longest_drought_days: int = 0
    total_drought_days: int = 0
    commit_merge_latency: list[CommitMergeLatencyEntry] = field(default_factory=list)
    merge_median_latency: float = 0.0
    merge_pct_within_7d: float = 0.0
    merge_pct_within_30d: float = 0.0
    release_intervals: list[ReleaseInterval] = field(default_factory=list)
    release_interval_distribution: list[HistogramBin] = field(default_factory=list)
    release_interval_mean: float = 0.0
    release_interval_median: float = 0.0
    release_interval_cv: float = 0.0
    release_interval_longest_gap: float = 0.0
    work_disposition: WorkDisposition = field(default_factory=WorkDisposition)


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
    waste_metrics: WasteMetrics | None = None


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
