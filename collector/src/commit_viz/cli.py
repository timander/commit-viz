from __future__ import annotations

import os
import time
from datetime import datetime, timezone
from pathlib import Path

import click

from commit_viz.config import load_config
from commit_viz.models import CollectedData, Metadata
from commit_viz.output import serialize
from commit_viz.sources.clone import ensure_repo
from commit_viz.sources.git import collect_git
from commit_viz.sources.stats import compute_statistics
from commit_viz.sources.change_flow import compute_change_flow


@click.group()
def main() -> None:
    """commit-viz: Visualize commit cycle time across branches."""


@main.command()
@click.option("--config", "config_path", required=True, help="Path to commit-viz.yaml")
@click.option("--output", "output_path", default="output.json", help="Output JSON path")
def collect(config_path: str, output_path: str) -> None:
    """Collect data from configured sources."""
    total_start = time.monotonic()
    cpu_count = os.cpu_count() or 1
    click.echo(f"Parallelization: {cpu_count} CPUs available")

    config = load_config(config_path)

    # Auto-clone if URL provided but repo not yet on disk
    if config.repo.url and config.repo.path:
        repo_dir = Path(config.repo.path)
        if not repo_dir.exists():
            t0 = time.monotonic()
            click.echo(f"Cloning {config.repo.url} into {repo_dir}...")
            ensure_repo(config.repo.url, repo_dir)
            click.echo(f"Clone complete. [{time.monotonic() - t0:.2f}s]")

    repo_name = config.repo.url or config.repo.path or "unknown"

    metadata = Metadata(
        repo=repo_name,
        date_range={"start": config.date_range.start, "end": config.date_range.end},
        generated_at=datetime.now(timezone.utc).isoformat(),
    )

    data = CollectedData(metadata=metadata)

    if config.sources.git:
        # Phase 1: Git data collection
        t0 = time.monotonic()
        click.echo(f"Collecting git data from {config.repo.path}...")
        branches, commits, merges = collect_git(config)
        data.branches = branches
        data.commits = commits
        data.merges = merges
        click.echo(f"  Found {len(commits)} commits, {len(branches)} branches, {len(merges)} merges [{time.monotonic() - t0:.2f}s]")

        # Phase 2: Statistics
        t0 = time.monotonic()
        click.echo("Computing statistics...")
        data.statistics = compute_statistics(commits)
        click.echo(f"  {data.statistics.unique_authors} authors, {data.statistics.commits_per_week} commits/week [{time.monotonic() - t0:.2f}s]")

        # Phase 3: Change flow metrics
        t0 = time.monotonic()
        click.echo("Computing change flow metrics...")
        data.statistics.change_flow = compute_change_flow(commits, merges, data.branches)
        click.echo(f"  {data.statistics.change_flow.drought_count} drought periods, "
                    f"{data.statistics.change_flow.branch_unmerged_count} unmerged branches [{time.monotonic() - t0:.2f}s]")

    # Phase 4: Serialization
    t0 = time.monotonic()
    output = Path(output_path)
    serialize(data, output)
    click.echo(f"Data written to {output} [{time.monotonic() - t0:.2f}s]")

    click.echo(f"Total elapsed: {time.monotonic() - total_start:.2f}s")


if __name__ == "__main__":
    main()
