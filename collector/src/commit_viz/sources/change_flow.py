from __future__ import annotations

import bisect
import math
from collections import Counter, defaultdict
from datetime import date, datetime, timedelta

from commit_viz.models import (
    Branch,
    BranchLifespan,
    ChangeFlowMetrics,
    Commit,
    CommitMergeLatencyEntry,
    CommitToReleaseDayEntry,
    DailyVelocity,
    DroughtPeriod,
    HistogramBin,
    Merge,
    ReleaseInterval,
    WorkDisposition,
    WorkDispositionSegment,
)


def _parse_ts(ts: str) -> datetime:
    return datetime.fromisoformat(ts)


def _to_date(ts: str) -> date:
    return _parse_ts(ts).date()


def _median(values: list[float]) -> float:
    if not values:
        return 0.0
    s = sorted(values)
    n = len(s)
    if n % 2 == 1:
        return s[n // 2]
    return (s[n // 2 - 1] + s[n // 2]) / 2.0


def _percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    s = sorted(values)
    idx = p / 100.0 * (len(s) - 1)
    lo = int(idx)
    hi = min(lo + 1, len(s) - 1)
    frac = idx - lo
    return s[lo] * (1 - frac) + s[hi] * frac


def _find_default_branch(branches: list[Branch]) -> str:
    for b in branches:
        if b.is_default:
            return b.name
    return "main"


def compute_change_flow(
    commits: list[Commit],
    merges: list[Merge],
    branches: list[Branch],
) -> ChangeFlowMetrics:
    if not commits:
        return ChangeFlowMetrics()

    default_branch = _find_default_branch(branches)

    # Build lookup structures
    merge_by_from_branch: dict[str, Merge] = {}
    for m in merges:
        # Keep the latest merge for each source branch
        if m.from_branch not in merge_by_from_branch or _parse_ts(m.timestamp) > _parse_ts(
            merge_by_from_branch[m.from_branch].timestamp
        ):
            merge_by_from_branch[m.from_branch] = m

    # Sorted tagged commits for release latency
    tagged_commits: list[tuple[datetime, str]] = sorted(
        (_parse_ts(c.timestamp), c.tags[0]) for c in commits if c.tags
    )
    tag_timestamps = [t[0] for t in tagged_commits]

    # 1. Commit-to-release latency (calendar heatmap data)
    commits_by_date: dict[date, list[Commit]] = defaultdict(list)
    for c in commits:
        commits_by_date[_to_date(c.timestamp)].append(c)

    all_release_latencies: list[float] = []
    commit_to_release_days: list[CommitToReleaseDayEntry] = []

    if commits_by_date:
        all_dates = sorted(commits_by_date.keys())
        for d in all_dates:
            day_commits = commits_by_date[d]
            latencies: list[float] = []
            unreleased = 0
            for c in day_commits:
                ct = _parse_ts(c.timestamp)
                idx = bisect.bisect_left(tag_timestamps, ct)
                if idx < len(tag_timestamps):
                    days_to_release = (tag_timestamps[idx] - ct).total_seconds() / 86400.0
                    latencies.append(days_to_release)
                    all_release_latencies.append(days_to_release)
                else:
                    unreleased += 1

            avg_lat = sum(latencies) / len(latencies) if latencies else -1.0
            commit_to_release_days.append(
                CommitToReleaseDayEntry(
                    date=d.isoformat(),
                    avg_days_to_release=round(avg_lat, 1),
                    commit_count=len(day_commits),
                    unreleased_count=unreleased,
                )
            )

    released_within_7d = sum(1 for v in all_release_latencies if v <= 7.0)

    # 2. Branch lifespans
    non_default_commits: dict[str, list[Commit]] = defaultdict(list)
    for c in commits:
        if c.branch != default_branch:
            non_default_commits[c.branch].append(c)

    branch_lifespans: list[BranchLifespan] = []
    all_lifespans: list[float] = []
    unmerged_count = 0
    longest_branch = 0.0

    for branch_name, bcommits in non_default_commits.items():
        timestamps = [_parse_ts(c.timestamp) for c in bcommits]
        first = min(timestamps)
        last = max(timestamps)
        merged = branch_name in merge_by_from_branch
        if merged:
            merge_time = _parse_ts(merge_by_from_branch[branch_name].timestamp)
            lifespan = (merge_time - first).total_seconds() / 86400.0
        else:
            lifespan = (last - first).total_seconds() / 86400.0
            unmerged_count += 1

        all_lifespans.append(lifespan)
        longest_branch = max(longest_branch, lifespan)

        branch_lifespans.append(
            BranchLifespan(
                branch=branch_name,
                first_commit=first.isoformat(),
                last_commit=last.isoformat(),
                lifespan_days=round(lifespan, 1),
                merged=merged,
                commit_count=len(bcommits),
            )
        )

    branch_lifespans.sort(key=lambda b: b.first_commit)

    # 3. Daily velocity + drought periods + rolling 7-day avg
    all_timestamps = [_parse_ts(c.timestamp) for c in commits]
    min_date = min(all_timestamps).date()
    max_date = max(all_timestamps).date()

    # Count commits per day and dominant category
    day_commit_counts: dict[date, int] = defaultdict(int)
    day_categories: dict[date, Counter] = defaultdict(Counter)
    for c in commits:
        d = _to_date(c.timestamp)
        day_commit_counts[d] += 1
        day_categories[d][c.category] += 1

    daily_velocity: list[DailyVelocity] = []
    day = min_date
    counts_list: list[int] = []
    while day <= max_date:
        count = day_commit_counts.get(day, 0)
        dom_cat = day_categories[day].most_common(1)[0][0] if day_categories[day] else "other"
        daily_velocity.append(
            DailyVelocity(date=day.isoformat(), count=count, dominant_category=dom_cat)
        )
        counts_list.append(count)
        day += timedelta(days=1)

    # Rolling 7-day average
    rolling_7day: list[dict[str, float | str]] = []
    for i, dv in enumerate(daily_velocity):
        start = max(0, i - 6)
        window = counts_list[start : i + 1]
        avg = sum(window) / len(window)
        rolling_7day.append({"date": dv.date, "avg": round(avg, 2)})

    # Drought periods (7+ consecutive zero-commit days)
    drought_periods: list[DroughtPeriod] = []
    drought_start: date | None = None
    drought_len = 0
    day = min_date
    while day <= max_date:
        if day_commit_counts.get(day, 0) == 0:
            if drought_start is None:
                drought_start = day
            drought_len += 1
        else:
            if drought_start is not None and drought_len >= 7:
                drought_periods.append(
                    DroughtPeriod(
                        start_date=drought_start.isoformat(),
                        end_date=(day - timedelta(days=1)).isoformat(),
                        duration_days=drought_len,
                    )
                )
            drought_start = None
            drought_len = 0
        day += timedelta(days=1)
    # Handle trailing drought
    if drought_start is not None and drought_len >= 7:
        drought_periods.append(
            DroughtPeriod(
                start_date=drought_start.isoformat(),
                end_date=max_date.isoformat(),
                duration_days=drought_len,
            )
        )

    longest_drought = max((d.duration_days for d in drought_periods), default=0)
    total_drought = sum(d.duration_days for d in drought_periods)

    # 4. Commit-to-merge latency
    commit_merge_latency: list[CommitMergeLatencyEntry] = []
    merge_latencies: list[float] = []

    for c in commits:
        if c.branch == default_branch:
            continue
        lines = c.insertions + c.deletions
        if c.branch in merge_by_from_branch:
            merge_ts = _parse_ts(merge_by_from_branch[c.branch].timestamp)
            commit_ts = _parse_ts(c.timestamp)
            days = (merge_ts - commit_ts).total_seconds() / 86400.0
            if days < 0:
                days = 0.0
            merge_latencies.append(days)
            commit_merge_latency.append(
                CommitMergeLatencyEntry(
                    sha=c.sha,
                    commit_date=c.timestamp,
                    days_to_merge=round(days, 2),
                    lines_changed=lines,
                    category=c.category,
                )
            )
        else:
            commit_merge_latency.append(
                CommitMergeLatencyEntry(
                    sha=c.sha,
                    commit_date=c.timestamp,
                    days_to_merge=None,
                    lines_changed=lines,
                    category=c.category,
                )
            )

    merged_within_7d = sum(1 for v in merge_latencies if v <= 7.0)
    merged_within_30d = sum(1 for v in merge_latencies if v <= 30.0)

    # 5. Release intervals
    release_intervals: list[ReleaseInterval] = []
    interval_values: list[float] = []

    if len(tagged_commits) >= 2:
        for i in range(1, len(tagged_commits)):
            prev_ts, _ = tagged_commits[i - 1]
            curr_ts, curr_tag = tagged_commits[i]
            days = (curr_ts - prev_ts).total_seconds() / 86400.0
            release_intervals.append(
                ReleaseInterval(
                    tag=curr_tag, date=curr_ts.isoformat(), days_since_previous=round(days, 1)
                )
            )
            interval_values.append(days)

    # Histogram bins for release intervals
    release_interval_distribution: list[HistogramBin] = []
    if interval_values:
        bins_def = [
            ("0-7d", 0, 7),
            ("7-14d", 7, 14),
            ("14-30d", 14, 30),
            ("30-60d", 30, 60),
            ("60-90d", 60, 90),
            ("90+d", 90, 999999),
        ]
        for label, lo, hi in bins_def:
            count = sum(1 for v in interval_values if lo <= v < hi)
            release_interval_distribution.append(
                HistogramBin(label=label, min_val=lo, max_val=hi, count=count)
            )

    ri_mean = sum(interval_values) / len(interval_values) if interval_values else 0.0
    ri_median = _median(interval_values)
    ri_stdev = (
        math.sqrt(sum((v - ri_mean) ** 2 for v in interval_values) / len(interval_values))
        if interval_values
        else 0.0
    )
    ri_cv = ri_stdev / ri_mean if ri_mean > 0 else 0.0
    ri_longest = max(interval_values, default=0.0)

    # 6. Work disposition
    fast_merged_lines = 0
    slow_merged_lines = 0
    unmerged_lines = 0
    fast_merged_commits = 0
    slow_merged_commits = 0
    unmerged_commits_count = 0
    segment_agg: dict[tuple[str, str], tuple[int, int]] = defaultdict(lambda: (0, 0))

    for c in commits:
        if c.branch == default_branch:
            continue
        lines = c.insertions + c.deletions
        if c.branch in merge_by_from_branch:
            merge_ts = _parse_ts(merge_by_from_branch[c.branch].timestamp)
            commit_ts = _parse_ts(c.timestamp)
            days = (merge_ts - commit_ts).total_seconds() / 86400.0
            if days <= 7.0:
                speed = "fast"
                fast_merged_lines += lines
                fast_merged_commits += 1
            else:
                speed = "slow"
                slow_merged_lines += lines
                slow_merged_commits += 1
        else:
            speed = "unmerged"
            unmerged_lines += lines
            unmerged_commits_count += 1

        prev_lines, prev_count = segment_agg[(c.category, speed)]
        segment_agg[(c.category, speed)] = (prev_lines + lines, prev_count + 1)

    segments = [
        WorkDispositionSegment(
            category=cat, merge_speed=speed, lines_changed=lines, commit_count=cnt
        )
        for (cat, speed), (lines, cnt) in sorted(segment_agg.items())
    ]

    return ChangeFlowMetrics(
        commit_to_release_days=commit_to_release_days,
        release_median_latency=round(_median(all_release_latencies), 1),
        release_p90_latency=round(_percentile(all_release_latencies, 90), 1),
        release_pct_within_7d=round(released_within_7d / len(all_release_latencies) * 100, 1)
        if all_release_latencies
        else 0.0,
        branch_lifespans=branch_lifespans,
        branch_median_lifespan=round(_median(all_lifespans), 1),
        branch_unmerged_count=unmerged_count,
        branch_longest_days=round(longest_branch, 1),
        daily_velocity=daily_velocity,
        rolling_7day_avg=rolling_7day,
        drought_periods=drought_periods,
        drought_count=len(drought_periods),
        longest_drought_days=longest_drought,
        total_drought_days=total_drought,
        commit_merge_latency=commit_merge_latency,
        merge_median_latency=round(_median(merge_latencies), 1),
        merge_pct_within_7d=round(merged_within_7d / len(merge_latencies) * 100, 1)
        if merge_latencies
        else 0.0,
        merge_pct_within_30d=round(merged_within_30d / len(merge_latencies) * 100, 1)
        if merge_latencies
        else 0.0,
        release_intervals=release_intervals,
        release_interval_distribution=release_interval_distribution,
        release_interval_mean=round(ri_mean, 1),
        release_interval_median=round(ri_median, 1),
        release_interval_cv=round(ri_cv, 2),
        release_interval_longest_gap=round(ri_longest, 1),
        work_disposition=WorkDisposition(
            fast_merged_lines=fast_merged_lines,
            slow_merged_lines=slow_merged_lines,
            unmerged_lines=unmerged_lines,
            fast_merged_commits=fast_merged_commits,
            slow_merged_commits=slow_merged_commits,
            unmerged_commits=unmerged_commits_count,
            segments=segments,
        ),
    )
