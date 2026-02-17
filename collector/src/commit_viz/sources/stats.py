from __future__ import annotations

import math
from collections import Counter
from datetime import datetime

from commit_viz.models import Commit, ReleaseCycleStats, Statistics


def compute_statistics(commits: list[Commit]) -> Statistics:
    if not commits:
        return Statistics()

    total = len(commits)

    # Date span
    timestamps = [datetime.fromisoformat(c.timestamp) for c in commits]
    earliest = min(timestamps)
    latest = max(timestamps)
    span = (latest - earliest).days
    weeks = max(span / 7.0, 1.0)

    # Unique authors
    authors = Counter(c.author for c in commits)
    top_authors: list[dict[str, int | str]] = [
        {"author": author, "commits": count} for author, count in authors.most_common(20)
    ]

    # By category
    by_category = Counter(c.category for c in commits)

    # By branch
    by_branch = Counter(c.branch for c in commits)

    # Release cycle analysis — look for tagged commits
    tagged_timestamps: list[datetime] = sorted(
        datetime.fromisoformat(c.timestamp) for c in commits if c.tags
    )

    release_cycles = ReleaseCycleStats()
    if len(tagged_timestamps) >= 2:
        intervals = [
            (tagged_timestamps[i + 1] - tagged_timestamps[i]).days
            for i in range(len(tagged_timestamps) - 1)
        ]
        n = len(intervals)
        mean = sum(intervals) / n
        variance = sum((x - mean) ** 2 for x in intervals) / n
        release_cycles = ReleaseCycleStats(
            count=len(tagged_timestamps),
            mean_days=round(mean, 1),
            min_days=float(min(intervals)),
            max_days=float(max(intervals)),
            stdev_days=round(math.sqrt(variance), 1),
        )

    return Statistics(
        total_commits=total,
        date_span_days=span,
        commits_per_week=round(total / weeks, 1),
        unique_authors=len(authors),
        by_category=dict(by_category),
        by_branch=dict(by_branch.most_common(50)),
        top_authors=top_authors,
        release_cycles=release_cycles,
    )
