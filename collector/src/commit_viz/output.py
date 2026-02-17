from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path

from commit_viz.models import CollectedData


def serialize(data: CollectedData, output_path: str | Path) -> None:
    output_path = Path(output_path)
    raw = asdict(data)
    with output_path.open("w") as f:
        json.dump(raw, f, indent=2)
