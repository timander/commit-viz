# commit-viz

Visualize commit cycle time across branches. Shows the flow (or impeded flow) of work through a repository, highlighting the difference between CI/CD (trunk-based) and feature-branching workflows.

## Architecture

- **collector/** — Python (uv) data collection from git, GitHub Actions, and Jira
- **renderer/** — Rust video rendering with swimlane timeline layout
- **schema/** — JSON Schema defining the interchange format between collector and renderer

## Quick Start

### Collect data

```bash
cd collector
uv sync
uv run commit-viz collect --config ../commit-viz.yaml.example
```

### Render video

```bash
cd renderer
cargo run -- --input ../output.json --output video.mp4
```

## Configuration

Copy `commit-viz.yaml.example` and edit to point at your repository. See `.env.example` for required secrets.

## License

MIT
