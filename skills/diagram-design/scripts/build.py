#!/usr/bin/env python3
"""Render, validate, and optionally capture one Diagram Design artifact."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
MAC_BROWSERS = (
    Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
    Path("/Applications/Chromium.app/Contents/MacOS/Chromium"),
)


def browser() -> str | None:
    for name in ("google-chrome", "chromium", "chromium-browser", "chrome"):
        found = shutil.which(name)
        if found:
            return found
    for candidate in MAC_BROWSERS:
        if candidate.is_file():
            return str(candidate)
    return None


def run(command: list[str]) -> int:
    return subprocess.run(command, check=False).returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("spec", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--inspect", type=Path, help="optional PNG inspection capture")
    parser.add_argument("--project-root", type=Path, default=Path.cwd())
    parser.add_argument("--width", type=int, default=1400)
    parser.add_argument("--height", type=int, default=900)
    args = parser.parse_args()

    render = [
        sys.executable,
        str(SCRIPTS / "render.py"),
        str(args.spec),
        "--output",
        str(args.output),
        "--project-root",
        str(args.project_root),
    ]
    if run(render):
        return 1
    if run([sys.executable, str(SCRIPTS / "check.py"), str(args.output)]):
        return 1

    if args.inspect:
        executable = browser()
        if not executable:
            print("error: Chrome or Chromium is required for --inspect", file=sys.stderr)
            return 1
        args.inspect.parent.mkdir(parents=True, exist_ok=True)
        screenshot = [
            executable,
            "--headless=new",
            "--hide-scrollbars",
            f"--window-size={args.width},{args.height}",
            f"--screenshot={args.inspect.resolve()}",
            args.output.resolve().as_uri(),
        ]
        capture = subprocess.run(screenshot, check=False, capture_output=True, text=True)
        if capture.returncode:
            print(capture.stdout, end="", file=sys.stderr)
            print(capture.stderr, end="", file=sys.stderr)
            return 1
        if not args.inspect.is_file() or not args.inspect.stat().st_size:
            print(f"error: inspection capture was not written: {args.inspect}", file=sys.stderr)
            return 1

    suffix = f", inspection: {args.inspect}" if args.inspect else ""
    print(f"Diagram Design build passed: {args.output}{suffix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
