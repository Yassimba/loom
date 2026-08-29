#!/usr/bin/env python3
"""Compile one explicit-geometry explanation bundle into every walkthrough artifact.

Author ``explanation.py`` with a top-level ``BUNDLE``. ``op`` is injected:

    BUNDLE = {
        "version": 1,
        "title": "Feature flow",
        "context": "Verified boundary (`src/main.py:10`).",
        "figures": [{
            "stem": "01-overview", "eyebrow": "Architecture",
            "title": "Entry reaches effect", "desc": "Architecture diagram.",
            "width": 760, "height": 320,
            "body": [
                op("hline", 208, 120, 336, 120),
                op("state", 64, 84, 144, 72, "Choose", "input"),
            ],
        }],
        "sections": [{
            "heading": "Overview", "claim": "One path reaches the effect.",
            "figure": "01-overview", "alt": "Architecture overview",
            "facts": ["Anchored fact (`src/main.py:10`)."],
        }],
        "result": "The entry reaches the effect.",
    }

Run:
    python3 bundle.py --repo /repo ai-docs/explanations/feature/explanation.py

Coordinates and paint order remain author-controlled. This compiler validates,
renders, rasterizes, checks anchors, builds the inlined HTML, and verifies the
exact artifact set. It diagnoses geometry; it never moves or reroutes anything.
"""
from __future__ import annotations

import argparse
import inspect
import re
import runpy
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import draw

SAFE_STEM = re.compile(r"^[0-9][0-9A-Za-z-]*$")
PRIMITIVES = {
    "node", "participant", "state", "start", "ring", "diamond", "oval",
    "step", "cls", "hline", "vline", "line", "path", "elbow", "uml",
    "label_above", "label_beside", "mult", "zone", "lifeline",
    "activation", "fragment", "callout", "legend", "sw_box", "sw_line",
    "sw_uml", "sw_ring", "sw_start", "sw_diamond", "sw_oval",
}
BOX_OPS = {"node", "participant", "state", "oval", "step", "cls"}


@dataclass(frozen=True)
class Operation:
    name: str
    args: tuple[Any, ...]
    kwargs: dict[str, Any]


def op(name: str, *args: Any, **kwargs: Any) -> Operation:
    """Record one existing draw.py primitive call for deferred validation."""
    return Operation(name, args, kwargs)


@dataclass(frozen=True)
class Rect:
    x: float
    y: float
    w: float
    h: float
    index: int
    role: str

    @property
    def right(self) -> float:
        return self.x + self.w

    @property
    def bottom(self) -> float:
        return self.y + self.h


@dataclass(frozen=True)
class Segment:
    x1: float
    y1: float
    x2: float
    y2: float
    index: int


def _overlap(a: Rect, b: Rect) -> tuple[float, float]:
    return min(a.right, b.right) - max(a.x, b.x), min(a.bottom, b.bottom) - max(a.y, b.y)


def _rect_for(operation: Operation, index: int) -> Rect | None:
    a = operation.args
    if operation.name in {"node", "participant", "state", "oval", "step"} and len(a) >= 4:
        return Rect(float(a[0]), float(a[1]), float(a[2]), float(a[3]), index, operation.name)
    if operation.name == "cls" and len(a) >= 5:
        attrs = a[4] or []
        ops = a[5] if len(a) > 5 else operation.kwargs.get("ops") or []
        h = 40 + (len(attrs) * 20 + 8 if attrs else 0) + (len(ops) * 20 + 8 if ops else 0)
        return Rect(float(a[0]), float(a[1]), float(a[2]), float(h), index, "cls")
    if operation.name == "callout" and len(a) >= 3:
        lines = a[2]
        width = max((len(str(line)) for line in lines), default=0) * 7.5
        return Rect(float(a[0]), float(a[1]) - 14, width, max(18, len(lines) * 18), index, "callout")
    if operation.name == "label_above" and len(a) >= 3:
        lines = operation.kwargs.get("lines") or [a[2]]
        width = max(draw.mono_w(str(line)) for line in lines)
        height = 12 * len(lines)
        return Rect(float(a[0]) - width / 2, float(a[1]) - 8 - height, width, height, index, "label")
    if operation.name == "label_beside" and len(a) >= 3:
        lines = operation.kwargs.get("lines") or [a[2]]
        width = max(draw.mono_w(str(line)) for line in lines)
        height = 12 * len(lines)
        anchor = operation.kwargs.get("anchor", "start")
        x = float(a[0]) - width if anchor == "end" else float(a[0])
        return Rect(x, draw.r4(float(a[1]) - height / 2), width, height, index, "label")
    return None


def _segments(operation: Operation, index: int) -> list[Segment]:
    a = operation.args
    points: list[tuple[float, float]] = []
    if operation.name in {"hline", "vline"} and len(a) >= 3:
        y2 = a[3] if len(a) > 3 and a[3] is not None else a[1]
        points = [(float(a[0]), float(a[1])), (float(a[2]), float(y2))]
    elif operation.name == "line" and len(a) >= 4:
        points = [(float(a[0]), float(a[1])), (float(a[2]), float(a[3]))]
    elif operation.name == "elbow" and a:
        points = [(float(x), float(y)) for x, y in a[0]]
    return [Segment(*left, *right, index) for left, right in zip(points, points[1:])]


def _crosses_rect(segment: Segment, rect: Rect) -> bool:
    epsilon = 0.5
    if abs(segment.y1 - segment.y2) < epsilon:
        y = segment.y1
        return rect.y + epsilon < y < rect.bottom - epsilon and min(segment.x1, segment.x2) < rect.right - epsilon and max(segment.x1, segment.x2) > rect.x + epsilon
    if abs(segment.x1 - segment.x2) < epsilon:
        x = segment.x1
        return rect.x + epsilon < x < rect.right - epsilon and min(segment.y1, segment.y2) < rect.bottom - epsilon and max(segment.y1, segment.y2) > rect.y + epsilon
    return False


def _geometry(figure: dict[str, Any]) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    width, height = float(figure["width"]), float(figure["height"])
    operations = figure["body"]
    rects = [rect for i, item in enumerate(operations, 1) if isinstance(item, Operation) and (rect := _rect_for(item, i))]
    boxes = [rect for rect in rects if rect.role in BOX_OPS]
    callouts = [rect for rect in rects if rect.role == "callout"]
    labels = [rect for rect in rects if rect.role == "label"]
    segments = [seg for i, item in enumerate(operations, 1) if isinstance(item, Operation) for seg in _segments(item, i)]

    for rect in rects:
        if rect.x < 0 or rect.y < 0 or rect.right > width or rect.bottom > height:
            errors.append(f"op {rect.index} {rect.role} is outside {width:g}x{height:g} viewBox")
    for i, left in enumerate(boxes):
        for right in boxes[i + 1:]:
            dx, dy = _overlap(left, right)
            if dx > 1 and dy > 1:
                errors.append(f"ops {left.index}/{right.index} boxes overlap by {dx:g}x{dy:g}px")
    for i, left in enumerate(labels):
        for right in labels[i + 1:]:
            dx, dy = _overlap(left, right)
            if dx > 1 and dy > 1:
                errors.append(f"ops {left.index}/{right.index} labels overlap by {dx:g}x{dy:g}px")
    for segment in segments:
        for box in boxes:
            if _crosses_rect(segment, box):
                errors.append(f"op {segment.index} connector crosses unrelated box op {box.index}")
        for callout in callouts:
            if _crosses_rect(segment, callout):
                errors.append(f"op {segment.index} connector crosses callout op {callout.index}")
    for i, left in enumerate(segments):
        for right in segments[i + 1:]:
            if left.index == right.index:
                continue
            if left.y1 == left.y2 == right.y1 == right.y2:
                overlap = min(max(left.x1, left.x2), max(right.x1, right.x2)) - max(min(left.x1, left.x2), min(right.x1, right.x2))
                if overlap > 8:
                    errors.append(f"ops {left.index}/{right.index} share {overlap:g}px of a horizontal route")
            elif left.x1 == left.x2 == right.x1 == right.x2:
                overlap = min(max(left.y1, left.y2), max(right.y1, right.y2)) - max(min(left.y1, left.y2), min(right.y1, right.y2))
                if overlap > 8:
                    errors.append(f"ops {left.index}/{right.index} share {overlap:g}px of a vertical route")

    for i, item in enumerate(operations, 1):
        if not isinstance(item, Operation):
            continue
        if item.name == "elbow" and item.args:
            for a, b in zip(item.args[0], item.args[0][1:]):
                if a[0] != b[0] and a[1] != b[1]:
                    errors.append(f"op {i} elbow has diagonal segment {a!r} -> {b!r}")
        if item.name in BOX_OPS:
            texts = [value for value in item.args[4:] if isinstance(value, str)]
            if any("\n" in text for text in texts):
                errors.append(f"op {i} {item.name} contains a literal newline in one SVG text node")
            rect = _rect_for(item, i)
            if rect and texts and len(texts[0]) * 7 > rect.w - 12:
                warnings.append(f"op {i} title may overflow its {rect.w:g}px box")
    return errors, warnings


def _resolve(value: Any, errors: list[str], location: str) -> Any:
    if isinstance(value, Operation):
        if value.name not in PRIMITIVES:
            errors.append(f"{location}: unknown primitive {value.name!r}")
            return ""
        function = getattr(draw, value.name)
        args = tuple(_resolve(arg, errors, location) for arg in value.args)
        kwargs = {key: _resolve(val, errors, location) for key, val in value.kwargs.items()}
        try:
            inspect.signature(function).bind(*args, **kwargs)
            result = function(*args, **kwargs)
        except Exception as exc:  # report every malformed operation together
            errors.append(f"{location}: {value.name}: {exc}")
            return ""
        return result[0] if isinstance(result, tuple) else result
    if isinstance(value, list):
        return [_resolve(item, errors, location) for item in value]
    if isinstance(value, tuple):
        return tuple(_resolve(item, errors, location) for item in value)
    if isinstance(value, dict):
        return {key: _resolve(item, errors, location) for key, item in value.items()}
    return value


def _walkthrough(bundle: dict[str, Any]) -> str:
    lines = [f"# {bundle['title']}", "", "## Context", "", bundle["context"].strip(), ""]
    for section in bundle["sections"]:
        lines += [f"## {section['heading']}", "", f"**{section['claim'].strip()}**", "", f"![{section['alt']}]" f"(diagrams/{section['figure']}.svg)", ""]
        lines += [f"- {fact.strip()}" for fact in section["facts"]]
        lines.append("")
    lines += ["## Result", "", bundle["result"].strip(), ""]
    return "\n".join(lines)


def _run(command: list[str], cwd: Path) -> None:
    print("+", " ".join(command))
    subprocess.run(command, cwd=cwd, check=True)


def compile_bundle(repo: Path, bundle_path: Path) -> None:
    for executable in ("pandoc", "rsvg-convert"):
        if not shutil.which(executable):
            raise SystemExit(f"missing required executable: {executable}")
    namespace = runpy.run_path(str(bundle_path), init_globals={"op": op})
    bundle = namespace.get("BUNDLE")
    if not isinstance(bundle, dict) or bundle.get("version") != 1:
        raise SystemExit("BUNDLE must be a mapping with version: 1")
    required = {"title", "context", "figures", "sections", "result"}
    missing = sorted(required - bundle.keys())
    if missing:
        raise SystemExit("BUNDLE missing: " + ", ".join(missing))

    errors: list[str] = []
    warnings: list[str] = []
    stems: list[str] = []
    rendered: list[tuple[dict[str, Any], str]] = []
    for number, figure in enumerate(bundle["figures"], 1):
        needed = {"stem", "eyebrow", "title", "desc", "width", "height", "body"}
        absent = sorted(needed - figure.keys())
        if absent:
            errors.append(f"figure {number} missing: {', '.join(absent)}")
            continue
        stem = figure["stem"]
        if not isinstance(stem, str) or not SAFE_STEM.fullmatch(stem):
            errors.append(f"figure {number}: unsafe stem {stem!r}")
            continue
        stems.append(stem)
        geometry_errors, geometry_warnings = _geometry(figure)
        errors.extend(f"{stem}: {finding}" for finding in geometry_errors)
        warnings.extend(f"{stem}: {finding}" for finding in geometry_warnings)
        body = _resolve(figure["body"], errors, stem)
        rendered.append((figure, "\n".join(str(item) for item in body)))
    if len(stems) != len(set(stems)):
        errors.append("figure stems must be unique")

    section_stems = [section.get("figure") for section in bundle["sections"]]
    if section_stems != stems:
        errors.append(f"section figure order {section_stems!r} must exactly match figures {stems!r}")
    for warning in warnings:
        print("WARN", warning)
    if errors:
        raise SystemExit("bundle validation failed:\n- " + "\n- ".join(errors))

    out = bundle_path.parent
    diagrams = out / "diagrams"
    diagrams.mkdir(parents=True, exist_ok=True)
    for figure, body in rendered:
        draw.write(
            diagrams / figure["stem"], figure["eyebrow"], figure["title"],
            figure["desc"], figure["width"], figure["height"], body,
            project=figure.get("project", ""), min_width=figure.get("min_width", 900),
        )
    walkthrough = out / "walkthrough.md"
    walkthrough.write_text(_walkthrough(bundle), encoding="utf-8")

    here = Path(__file__).resolve().parent
    _run([sys.executable, str(here / "check-anchors.py"), str(repo), str(out / "brief.md")], repo)
    _run([str(here / "check-figures.sh"), str(diagrams)], repo)
    _run([sys.executable, str(here / "check-anchors.py"), str(repo), str(walkthrough), "--quiet"], repo)
    _run([sys.executable, str(here / "build-html.py"), str(walkthrough)], repo)

    expected = set(stems)
    for suffix, folder in ((".html", diagrams), (".svg", diagrams), (".png", diagrams / "png")):
        actual = {path.stem for path in folder.glob(f"*{suffix}")}
        if actual != expected:
            raise SystemExit(f"{suffix} artifact set {sorted(actual)!r} != declared {sorted(expected)!r}")
    source = walkthrough.read_text(encoding="utf-8")
    for stem in stems:
        if source.count(f"diagrams/{stem}.svg") != 1:
            raise SystemExit(f"walkthrough must embed {stem}.svg exactly once")
    print(f"OK bundle: {len(stems)} figure(s), exact artifacts, anchors, PNGs, and inlined HTML")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("bundle", type=Path)
    args = parser.parse_args()
    compile_bundle(args.repo.resolve(), args.bundle.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
