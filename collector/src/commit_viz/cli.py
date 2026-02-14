from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

import click

from commit_viz.config import load_config
from commit_viz.models import CollectedData, Metadata
from commit_viz.output import serialize
from commit_viz.sources.git import collect_git


@click.group()
def main() -> None:
    """commit-viz: Visualize commit cycle time across branches."""


@main.command()
@click.option("--config", "config_path", required=True, help="Path to commit-viz.yaml")
@click.option("--output", "output_path", default="output.json", help="Output JSON path")
def collect(config_path: str, output_path: str) -> None:
    """Collect data from configured sources."""
    config = load_config(config_path)

    repo_name = config.repo.url or config.repo.path or "unknown"

    metadata = Metadata(
        repo=repo_name,
        date_range={"start": config.date_range.start, "end": config.date_range.end},
        generated_at=datetime.now(timezone.utc).isoformat(),
    )

    data = CollectedData(metadata=metadata)

    if config.sources.git:
        click.echo(f"Collecting git data from {config.repo.path}...")
        branches, commits, merges = collect_git(config)
        data.branches = branches
        data.commits = commits
        data.merges = merges
        click.echo(f"  Found {len(commits)} commits, {len(branches)} branches, {len(merges)} merges")

    output = Path(output_path)
    serialize(data, output)
    click.echo(f"Data written to {output}")


if __name__ == "__main__":
    main()
