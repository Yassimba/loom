#!/usr/bin/env python3
"""Resolve the effective Diagram Design profile for rendering."""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse

SKILL_DIR = Path(__file__).resolve().parent.parent
STYLE_GUIDE = SKILL_DIR / "references" / "style-guide.md"
SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
MARKER_RE = re.compile(r"[ \t]*profile:[ \t]*([a-z0-9][a-z0-9-]{0,63})[ \t]*(?:\n)?")
FONT_LINK_RE = re.compile(r'<link href="(https://fonts\.googleapis\.com/css2\?[^"<>]+)" rel="stylesheet">')
SAFE_FONT_RE = re.compile(r"^[A-Za-z0-9 ._-]+$")


@dataclass(frozen=True)
class Profile:
    source: Path
    tokens: dict[str, str]
    fonts: dict[str, str]
    font_href: str | None
    warnings: tuple[str, ...] = ()


def table_rows(text: str, heading: str) -> list[list[str]]:
    start = text.find(heading)
    if start < 0:
        return []
    rows: list[list[str]] = []
    for line in text[start + len(heading):].splitlines():
        if line.startswith("##"):
            break
        if not line.lstrip().startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if cells and not set(cells[0]) <= {"-", ":"}:
            rows.append(cells)
    return rows


def code_value(cell: str) -> str:
    match = re.search(r"`([^`]+)`", cell)
    return match.group(1).strip() if match else cell.strip().strip("*`")


def parse_tokens(text: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for cells in table_rows(text, "### Semantic roles"):
        if len(cells) < 3:
            continue
        name = code_value(cells[0])
        value = code_value(cells[2])
        if name and value and name != "Role":
            result[name] = value
    return result


def parse_fonts(text: str) -> dict[str, str]:
    roles: dict[str, str] = {}
    for cells in table_rows(text, "## Typography"):
        if len(cells) < 2:
            continue
        role = code_value(cells[0])
        family = code_value(cells[1]).replace("*", "").strip()
        family = re.sub(r"\s+\((?:sans|serif|mono)\)$", "", family, flags=re.IGNORECASE)
        if role and family and role != "Role":
            roles[role] = family
    fonts = {
        "sans": roles.get("node-name", "Geist"),
        "serif": roles.get("title", "Instrument Serif"),
        "mono": roles.get("sublabel", "Geist Mono"),
    }
    for role, family in fonts.items():
        if not SAFE_FONT_RE.fullmatch(family):
            raise ValueError(f"profile has unsafe {role} font family {family!r}")
    return fonts


def parse_font_href(text: str) -> str | None:
    match = FONT_LINK_RE.search(text)
    if not match:
        return None
    href = match.group(1).replace("&amp;", "&")
    parsed = urlparse(href)
    if parsed.scheme != "https" or parsed.hostname != "fonts.googleapis.com" or parsed.path != "/css2":
        raise ValueError("profile font stylesheet is outside the Google Fonts allowlist")
    return href


def load_profile(path: Path, defaults: dict[str, str], warnings: list[str]) -> Profile:
    text = path.read_text(encoding="utf-8")
    selected = parse_tokens(text)
    if not selected:
        raise ValueError(f"profile lacks a Semantic roles table: {path}")
    missing = sorted(set(defaults) - set(selected))
    if missing:
        selected.update({name: defaults[name] for name in missing})
        warnings.append(f"backfilled profile roles from shipped defaults: {', '.join(missing)}")
    return Profile(path, selected, parse_fonts(text), parse_font_href(text), tuple(warnings))


def resolve_profile(project_root: Path, home: Path | None = None) -> Profile:
    """Resolve marker-first, then the installed working style guide."""
    project_root = project_root.resolve()
    defaults = parse_tokens(STYLE_GUIDE.read_text(encoding="utf-8"))
    if not defaults:
        raise ValueError("shipped style guide lacks Semantic roles")
    warnings: list[str] = []
    marker = project_root / ".diagram-design"
    selected = STYLE_GUIDE
    if marker.is_file():
        marker_text = marker.read_text(encoding="utf-8")
        match = MARKER_RE.fullmatch(marker_text)
        if match and SLUG_RE.fullmatch(match.group(1)):
            library = (home or Path.home()) / ".diagram-design" / "profiles"
            slug = match.group(1)
            selected = library / f"{slug}.md"
            if not selected.is_file() and slug == "default":
                selected = STYLE_GUIDE
                warnings.append("default profile snapshot is missing; used shipped defaults")
            elif not selected.is_file():
                raise ValueError(f"diagram profile {slug!r} is missing: {selected}")
        else:
            warnings.append("ignored malformed .diagram-design marker")
    return load_profile(selected, defaults, warnings)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project-root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    try:
        profile = resolve_profile(args.project_root)
    except (OSError, UnicodeError, ValueError) as exc:
        print(json.dumps({"error": str(exc)}))
        return 1
    print(json.dumps({
        "source": str(profile.source),
        "tokens": profile.tokens,
        "fonts": profile.fonts,
        "fontHref": profile.font_href,
        "warnings": profile.warnings,
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
