#!/usr/bin/env python3
"""Run the installed Diagram Design checks with one command."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent


def main() -> int:
    if len(sys.argv) < 2:
        print(f"usage: {Path(sys.argv[0]).name} FILE [FILE ...]", file=sys.stderr)
        return 2
    files = sys.argv[1:]
    commands = [
        [sys.executable, str(SCRIPTS / "self_check.py"), *files],
        [sys.executable, str(SCRIPTS / "verify-geometry.py"), *files],
    ]
    results = [subprocess.run(command, check=False).returncode for command in commands]
    return int(any(results))


if __name__ == "__main__":
    raise SystemExit(main())
