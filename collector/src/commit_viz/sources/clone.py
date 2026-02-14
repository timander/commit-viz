from __future__ import annotations

import subprocess
from pathlib import Path


def ensure_repo(url: str, target_path: Path) -> Path:
    """Clone a git repository if not already present.

    Uses --filter=blob:none for a partial (blobless) clone to save bandwidth.
    Returns the path to the cloned repository.
    """
    if target_path.exists() and (target_path / ".git").exists():
        return target_path

    target_path.parent.mkdir(parents=True, exist_ok=True)

    subprocess.run(
        [
            "git",
            "clone",
            "--filter=blob:none",
            url,
            str(target_path),
        ],
        check=True,
    )

    return target_path
