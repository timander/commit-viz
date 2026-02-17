from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path

import yaml
from dotenv import load_dotenv


@dataclass
class JiraConfig:
    enabled: bool = False
    projects: list[str] = field(default_factory=list)
    base_url: str = ""


@dataclass
class SourcesConfig:
    git: bool = True
    github_actions: bool = False
    jira: JiraConfig = field(default_factory=JiraConfig)


@dataclass
class DateRange:
    start: str = ""
    end: str = ""


@dataclass
class VideoSpeedConfig:
    mode: str = "per_month"  # per_day, per_week, per_month, duration
    value: float = 1.0  # seconds per unit, or total duration in seconds


@dataclass
class RenderingConfig:
    style: str = "timeline"
    output: str = "output.mp4"
    fps: int = 30
    resolution: tuple[int, int] = (1920, 1080)
    video_speed: VideoSpeedConfig = field(default_factory=VideoSpeedConfig)


@dataclass
class RepoConfig:
    path: str | None = None
    url: str | None = None


@dataclass
class Config:
    repo: RepoConfig = field(default_factory=RepoConfig)
    date_range: DateRange = field(default_factory=DateRange)
    sources: SourcesConfig = field(default_factory=SourcesConfig)
    rendering: RenderingConfig = field(default_factory=RenderingConfig)

    # Secrets from environment
    github_token: str | None = None
    jira_api_token: str | None = None
    jira_user_email: str | None = None


def load_config(config_path: str | Path) -> Config:
    load_dotenv()

    config_path = Path(config_path)
    with config_path.open() as f:
        raw = yaml.safe_load(f)

    raw = raw or {}

    repo_raw = raw.get("repo", {})
    repo_path = repo_raw.get("path")
    if repo_path is not None:
        repo_path = str((config_path.parent / repo_path).resolve())
    repo = RepoConfig(path=repo_path, url=repo_raw.get("url"))

    dr_raw = raw.get("date_range", {})
    date_range = DateRange(start=dr_raw.get("start", ""), end=dr_raw.get("end", ""))

    src_raw = raw.get("sources", {})
    jira_raw = src_raw.get("jira", {})
    jira = JiraConfig(
        enabled=jira_raw.get("enabled", False),
        projects=jira_raw.get("projects", []),
        base_url=jira_raw.get("base_url", ""),
    )
    sources = SourcesConfig(
        git=src_raw.get("git", True),
        github_actions=src_raw.get("github_actions", False),
        jira=jira,
    )

    rend_raw = raw.get("rendering", {})
    res = rend_raw.get("resolution", [1920, 1080])
    speed_raw = rend_raw.get("video_speed", {})
    video_speed = VideoSpeedConfig(
        mode=speed_raw.get("mode", "per_month"),
        value=float(speed_raw.get("value", 1.0)),
    )
    rendering = RenderingConfig(
        style=rend_raw.get("style", "timeline"),
        output=rend_raw.get("output", "output.mp4"),
        fps=rend_raw.get("fps", 30),
        resolution=(res[0], res[1]),
        video_speed=video_speed,
    )

    return Config(
        repo=repo,
        date_range=date_range,
        sources=sources,
        rendering=rendering,
        github_token=os.environ.get("GITHUB_TOKEN"),
        jira_api_token=os.environ.get("JIRA_API_TOKEN"),
        jira_user_email=os.environ.get("JIRA_USER_EMAIL"),
    )
