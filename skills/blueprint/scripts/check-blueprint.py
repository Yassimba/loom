#!/usr/bin/env python3
"""Validate a Blueprint artifact and optionally lock its approved plan."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

FIGURE_ROW_RE = re.compile(r"^\|(.+?)\|\s*(DRAW|SKIP)\s*\|(.+?)\|(.+?)\|\s*$")
DIRECTIVE_INFO_RE = re.compile(r'^plannotator-svg path="([^"\r\n]+\.svg)"$')
DATA_CODE_RE = re.compile(r"\bdata-code\s*=\s*([\"'])(.*?)\1")
REQUIRED_CHANGE_FIELDS = ("id", "class", "target", "current", "projected", "reason", "verification")
REQUIRED_HANDOFF_FIELDS = ("outcome",)
REQUIRED_EVIDENCE_FIELDS = ("summary", "sources")
CHANGE_CLASSES = {"added", "removed", "changed"}
MAX_SVG_BYTES = 2 * 1024 * 1024
ROOT_ARTIFACTS = {
    "approval.json", "approved-plan.md", "brief.md", "changes.json",
    "evidence.json", "figure-selection.md", "plan.md", "overlay.json", "context.json",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("blueprint_dir", type=Path)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--lock", action="store_true")
    parser.add_argument("--baseline", action="store_true", help="print the source baseline hash, excluding this Blueprint directory")
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_git(repo_root: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo_root), *args], stderr=subprocess.STDOUT)


def selection_rows(path: Path) -> list[tuple[str, str, str, str]]:
    rows: list[tuple[str, str, str, str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        match = FIGURE_ROW_RE.match(line)
        if not match:
            continue
        type_name, verdict, figure, value = (part.strip() for part in match.groups())
        if type_name != "Type":
            rows.append((type_name, verdict, figure.strip("`"), value))
    return rows


def plan_directives(plan: str) -> tuple[list[str], list[str]]:
    paths: list[str] = []
    errors: list[str] = []
    lines = plan.splitlines()
    index = 0
    while index < len(lines):
        opener = re.match(r"^[ \t]*(`{3,})(.*)$", lines[index])
        if not opener:
            index += 1
            continue
        ticks, info = opener.groups()
        body: list[str] = []
        index += 1
        while index < len(lines) and not re.match(rf"^[ \t]*`{{{len(ticks)},}}[ \t]*$", lines[index]):
            body.append(lines[index])
            index += 1
        closed = index < len(lines)
        index += 1
        if not info.strip().startswith("plannotator-svg"):
            continue
        match = DIRECTIVE_INFO_RE.fullmatch(info.strip())
        if not closed or not match or any(line.strip() for line in body):
            errors.append("every plannotator-svg fence must use the exact empty-body directive grammar")
            continue
        paths.append(match.group(1))
    return paths, errors


def valid_svg_path(relative: str) -> bool:
    return bool(relative) and not (
        "\0" in relative
        or relative.startswith("/")
        or re.match(r"^[A-Za-z]:[\\/]", relative)
        or "\\" in relative
        or "?" in relative
        or "#" in relative
        or ".." in relative.split("/")
        or not relative.lower().endswith(".svg")
    )


def contained_file(repo_root: Path, relative: str) -> Path:
    if not relative or Path(relative).is_absolute() or "\\" in relative:
        raise ValueError(f"path must be repository-relative: {relative}")
    candidate = (repo_root / relative).resolve(strict=True)
    root = repo_root.resolve(strict=True)
    if not candidate.is_relative_to(root) or not candidate.is_file():
        raise ValueError(f"path leaves the repository or is not a file: {relative}")
    return candidate


def validate_source_range(value: str, repo_root: Path, errors: list[str], label: str) -> None:
    path, separator, line_range = value.rpartition(":")
    if not separator or not re.fullmatch(r"[1-9]\d*-[1-9]\d*", line_range):
        errors.append(f"{label}: invalid source range: {value}")
        return
    try:
        source = contained_file(repo_root, path)
        start, end = (int(part) for part in line_range.split("-"))
        line_count = len(source.read_text(encoding="utf-8", errors="replace").splitlines())
        if start > end or end > line_count:
            errors.append(f"{label}: source range {value} exceeds {line_count} lines")
    except (OSError, ValueError) as error:
        errors.append(f"{label}: {error}")


def validate_code_bindings(svg: Path, repo_root: Path, errors: list[str]) -> None:
    text = svg.read_text(encoding="utf-8")
    for match in DATA_CODE_RE.finditer(text):
        for binding in match.group(2).split(","):
            validate_source_range(binding.strip(), repo_root, errors, svg.name)


def baseline_sha256(repo_root: Path, exclude: Path | None = None) -> str:
    digest = hashlib.sha256()
    digest.update(run_git(repo_root, "rev-parse", "HEAD"))
    paths = ["--", ".", f":(exclude){exclude.relative_to(repo_root)}"] if exclude else []
    digest.update(run_git(repo_root, "diff", "--binary", "HEAD", *paths))
    status = run_git(repo_root, "status", "--porcelain=v1", "-z", "--untracked-files=all", *paths)
    digest.update(status)
    for entry in status.split(b"\0"):
        if not entry.startswith(b"?? "):
            continue
        relative = entry[3:].decode("utf-8", errors="surrogateescape")
        source = repo_root / relative
        if source.is_file():
            digest.update(relative.encode("utf-8", errors="surrogateescape"))
            digest.update(source.read_bytes())
    return digest.hexdigest()


def retained_artifacts(directory: Path) -> tuple[set[str], list[str]]:
    retained_files = {
        path.relative_to(directory).as_posix()
        for path in directory.rglob("*")
        if path.is_file() or path.is_symlink()
    }
    invalid = sorted(
        relative for relative in retained_files
        if relative not in ROOT_ARTIFACTS
        and not (relative.startswith("diagrams/") and relative.lower().endswith((".svg", ".mmd")))
    )
    return retained_files, invalid


def validate(directory: Path, repo_root: Path) -> list[str]:
    if (directory / "overlay.json").exists():
        return validate_compact(directory, repo_root)
    errors: list[str] = []
    required = ["brief.md", "evidence.json", "changes.json", "figure-selection.md", "plan.md"]
    for name in required:
        if not (directory / name).is_file():
            errors.append(f"missing {name}")
    if errors:
        return errors

    try:
        evidence = json.loads((directory / "evidence.json").read_text(encoding="utf-8"))
        if not isinstance(evidence, dict):
            raise ValueError("root must be an object")
        missing = [field for field in REQUIRED_EVIDENCE_FIELDS if not evidence.get(field)]
        if missing:
            errors.append(f"evidence.json missing non-empty fields: {missing}")
        sources = evidence.get("sources")
        if not isinstance(sources, list) or not sources:
            errors.append("evidence.json sources must be a non-empty array")
        else:
            for index, item in enumerate(sources):
                if not isinstance(item, dict):
                    errors.append(f"evidence.json sources[{index}] must be an object")
                    continue
                source = item.get("source")
                finding = item.get("finding")
                if not isinstance(source, str) or not source.strip():
                    errors.append(f"evidence.json sources[{index}] needs a source range")
                else:
                    validate_source_range(source.strip(), repo_root, errors, f"evidence.json sources[{index}]")
                if not isinstance(finding, str) or not finding.strip():
                    errors.append(f"evidence.json sources[{index}] needs a finding")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        errors.append(f"evidence.json: {error}")

    try:
        rows = selection_rows(directory / "figure-selection.md")
        if not rows:
            errors.append("figure-selection needs at least one verdict")
        selected_types = [row[0] for row in rows]
        if len(selected_types) != len(set(selected_types)):
            errors.append("figure-selection types must be unique")
    except (OSError, ValueError) as error:
        rows = []
        errors.append(f"figure-selection: {error}")

    retained_files, invalid_retained = retained_artifacts(directory)
    if invalid_retained:
        errors.append(f"temporary or unsupported artifacts must not be retained: {invalid_retained}")

    draw_figures: list[Path] = []
    for type_name, verdict, figure, value in rows:
        if not value or value == "—":
            errors.append(f"{type_name}: verdict needs a value explanation")
        if verdict == "DRAW":
            if figure == "—":
                errors.append(f"{type_name}: DRAW needs an SVG path")
                continue
            svg = directory / figure
            if not svg.is_file():
                errors.append(f"{type_name}: missing {figure}")
            else:
                if svg.stat().st_size > MAX_SVG_BYTES:
                    errors.append(f"{type_name}: {figure} exceeds {MAX_SVG_BYTES} bytes")
                draw_figures.append(svg.resolve())
        elif figure != "—":
            errors.append(f"{type_name}: SKIP figure cell must be —")
    if not draw_figures:
        errors.append("figure-selection needs at least one DRAW verdict")
    elif len(draw_figures) != len(set(draw_figures)):
        errors.append("each DRAW verdict needs a distinct SVG")

    plan = (directory / "plan.md").read_text(encoding="utf-8")
    directive_paths, directive_errors = plan_directives(plan)
    errors.extend(directive_errors)
    directive_figures: list[Path] = []
    for relative in directive_paths:
        if not valid_svg_path(relative):
            errors.append(f"path must match the runtime repository SVG grammar: {relative}")
            continue
        try:
            directive_figures.append(contained_file(repo_root, relative))
        except (OSError, ValueError) as error:
            errors.append(str(error))
    if sorted(directive_figures) != sorted(draw_figures):
        errors.append("plan directives must reference every DRAW SVG exactly once and no others")
    retained_diagrams = {
        (directory / relative).resolve()
        for relative in retained_files
        if relative.startswith("diagrams/") and relative.lower().endswith(".svg")
    }
    if retained_diagrams != set(draw_figures):
        errors.append("diagrams must contain exactly the SVGs selected with DRAW")

    try:
        ledger = json.loads((directory / "changes.json").read_text(encoding="utf-8"))
        changes = ledger.get("changes") if isinstance(ledger, dict) else None
        if not isinstance(changes, list) or not changes:
            raise ValueError("changes must be a non-empty array")
        handoff = ledger.get("handoff")
        if not isinstance(handoff, dict):
            errors.append("changes.json handoff must be an object")
        else:
            missing = [
                field for field in REQUIRED_HANDOFF_FIELDS
                if not isinstance(handoff.get(field), str) or not handoff[field].strip()
            ]
            if missing:
                errors.append(f"changes.json handoff missing non-empty fields: {missing}")
            acceptance = handoff.get("acceptanceCriteria")
            if not isinstance(acceptance, list) or not acceptance or not all(
                isinstance(item, str) and item.strip() for item in acceptance
            ):
                errors.append("changes.json handoff acceptanceCriteria must be a non-empty string array")
            risks = handoff.get("unresolvedRisks")
            if not isinstance(risks, list) or not all(isinstance(item, str) and item.strip() for item in risks):
                errors.append("changes.json handoff unresolvedRisks must be a string array")

        ids: list[str] = []
        for index, change in enumerate(changes):
            if not isinstance(change, dict):
                errors.append(f"changes[{index}] must be an object")
                continue
            missing = [
                field for field in REQUIRED_CHANGE_FIELDS
                if not isinstance(change.get(field), str) or not change[field].strip()
            ]
            if missing:
                errors.append(f"changes[{index}] missing non-empty fields: {missing}")
                continue
            if change["class"] not in CHANGE_CLASSES:
                errors.append(f"{change['id']}: class must be one of {sorted(CHANGE_CLASSES)}")
            ids.append(change["id"])
        if len(ids) != len(set(ids)):
            errors.append("change ids must be unique")

        for svg in draw_figures:
            text = svg.read_text(encoding="utf-8")
            if "viewBox=" not in text:
                errors.append(f"{svg.relative_to(directory)} lacks viewBox")
            validate_code_bindings(svg, repo_root, errors)
        unlisted = [
            change_id for change_id in ids
            if not re.search(rf"(?<![A-Za-z0-9_]){re.escape(change_id)}(?![A-Za-z0-9_])", plan)
        ]
        if unlisted:
            errors.append(f"plan searchable ledger is missing change ids: {unlisted}")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        errors.append(f"changes.json: {error}")

    errors.extend(validate_approval(directory, repo_root))
    return errors


def validate_approval(directory: Path, repo_root: Path) -> list[str]:
    errors: list[str] = []
    approval = directory / "approval.json"
    approved_plan = directory / "approved-plan.md"
    if approval.exists() != approved_plan.exists():
        errors.append("approval.json and approved-plan.md must exist together")
    elif approval.exists():
        try:
            record = json.loads(approval.read_text(encoding="utf-8"))
            expected_plan_path = str((directory / "plan.md").relative_to(repo_root))
            if record.get("planPath") != expected_plan_path:
                errors.append("approval.json planPath does not identify plan.md")
            if record.get("planSha256") != sha256_file(approved_plan):
                errors.append("approval.json planSha256 does not match approved-plan.md")
            if approved_plan.read_bytes() != (directory / "plan.md").read_bytes():
                errors.append("plan.md differs from the locked approved-plan.md")
            if not re.fullmatch(r"[0-9a-f]{40,64}", str(record.get("head", ""))):
                errors.append("approval.json head is not a Git object id")
            if not re.fullmatch(r"[0-9a-f]{64}", str(record.get("baselineSha256", ""))):
                errors.append("approval.json baselineSha256 is invalid")
            approved_at = datetime.fromisoformat(str(record.get("approvedAt", "")))
            if approved_at.utcoffset() is None:
                errors.append("approval.json approvedAt must include a timezone")
            if (directory / "overlay.json").exists() and record.get("artifactSha256") != artifact_hashes(directory):
                errors.append("approved artifacts changed: overlay, context, or figures differ from the lock")
        except (OSError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"approval.json: {error}")

    return errors


def artifact_hashes(directory: Path) -> dict[str, str]:
    files, invalid = retained_artifacts(directory)
    if invalid:
        raise ValueError(f"unsupported artifacts: {invalid}")
    return {relative: sha256_file(contained_file(directory, relative))
            for relative in sorted(files) if relative not in {"approval.json", "approved-plan.md"}}


def validate_compact(directory: Path, repo_root: Path) -> list[str]:
    errors: list[str] = []
    try:
        overlay = json.loads((directory / "overlay.json").read_text(encoding="utf-8"))
        if not isinstance(overlay, dict) or overlay.get("version") != 2:
            raise ValueError("overlay.json requires version 2")
        plan = (directory / "plan.md").read_text(encoding="utf-8")
        for heading in ("Intent", "Acceptance criteria", "Changes", "Implementation", "Verification", "Risks"):
            if not re.search(rf"^## {re.escape(heading)}\s*$", plan, re.M | re.I):
                errors.append(f"plan.md needs a {heading} section")
        if not re.search(r"\bC[1-9]\d*\b", plan):
            errors.append("plan.md needs stable C1/C2 change IDs")
        target = overlay.get("target", {})
        if not isinstance(target, dict) or not re.fullmatch(r"[0-9a-f]{40,64}", str(target.get("head", ""))):
            errors.append("overlay target needs a full head commit ID")
        elif not (directory / "approval.json").exists():
            if target["head"] != run_git(repo_root, "rev-parse", "HEAD").decode().strip():
                errors.append("overlay target differs from review HEAD; inspect drift before approval")
            if target.get("baselineSha256") != baseline_sha256(repo_root, directory):
                errors.append("overlay target baseline changed; inspect source drift and capture --baseline again")
        atlas = overlay.get("atlas")
        if atlas is not None:
            if not isinstance(atlas, dict) or atlas.get("snapshot") != "context.json" or not atlas.get("topics"):
                raise ValueError("atlas needs selected topics and context.json snapshot")
            snapshot = json.loads(contained_file(directory, "context.json").read_text(encoding="utf-8"))
            if {row["id"] for row in snapshot["topics"]} != set(atlas["topics"]):
                errors.append("context snapshot does not match selected atlas topics")
            if not snapshot.get("repositories") or any(not re.fullmatch(r"[0-9a-f]{40,64}", row["commit"])
                                                       for row in snapshot["repositories"].values()):
                errors.append("context snapshot needs pinned repositories")
        figures = overlay.get("figures")
        if not isinstance(figures, list):
            raise ValueError("overlay figures must be a list")
        paths, directive_errors = plan_directives(plan)
        errors.extend(directive_errors)
        selected = []
        ids = []
        for figure in figures:
            ids.append(figure["id"])
            relative = figure["output"]
            if not relative.startswith("diagrams/") or not valid_svg_path(relative):
                raise ValueError(f"invalid figure output: {relative}")
            svg = contained_file(directory, relative)
            selected.append(svg)
            content = svg.read_text(encoding="utf-8")
            if svg.stat().st_size > MAX_SVG_BYTES or "viewBox=" not in content:
                errors.append(f"{relative}: SVG needs viewBox and must fit the size limit")
            if not (directory / "approval.json").exists():
                validate_code_bindings(svg, repo_root, errors)
            if "mermaid" in figure:
                mermaid = figure["mermaid"]
                if not mermaid.startswith("diagrams/") or not mermaid.endswith(".mmd"):
                    raise ValueError("Mermaid source must be diagrams/*.mmd")
                contained_file(directory, mermaid)
            elif atlas is None or not figure.get("source") or not isinstance(figure.get("patch"), dict):
                errors.append(f"{relative}: needs an atlas source/patch or Mermaid source")
        if len(ids) != len(set(ids)) or len(selected) != len(set(selected)):
            errors.append("figure IDs and outputs must be unique")
        actual = []
        for relative in paths:
            if not valid_svg_path(relative):
                raise ValueError(f"invalid SVG directive: {relative}")
            actual.append(contained_file(repo_root, relative))
        if sorted(actual) != sorted(selected):
            errors.append("plan must reference each selected SVG exactly once")
        retained, invalid = retained_artifacts(directory)
        if invalid:
            errors.append(f"unsupported artifacts: {invalid}")
        if {contained_file(directory, path) for path in retained if path.endswith(".svg")} != set(selected):
            errors.append("retain only selected SVG outputs")
        errors.extend(validate_approval(directory, repo_root))
    except (OSError, ValueError, TypeError, KeyError, subprocess.CalledProcessError) as error:
        errors.append(f"compact Blueprint: {error}")
    return errors


def lock(directory: Path, repo_root: Path) -> None:
    approval = directory / "approval.json"
    approved_plan = directory / "approved-plan.md"
    if approval.exists() or approved_plan.exists():
        raise ValueError("approval is already locked; preserve it and create a new Blueprint revision")
    baseline = baseline_sha256(repo_root, directory if (directory / "overlay.json").exists() else None)
    shutil.copyfile(directory / "plan.md", approved_plan)
    record = {
        "planPath": str((directory / "plan.md").relative_to(repo_root)),
        "planSha256": sha256_file(approved_plan),
        "head": run_git(repo_root, "rev-parse", "HEAD").decode().strip(),
        "baselineSha256": baseline,
        "approvedAt": datetime.now(timezone.utc).isoformat(),
    }
    if (directory / "overlay.json").exists():
        record["artifactSha256"] = artifact_hashes(directory)
    temporary = approval.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    temporary.replace(approval)


def main() -> int:
    args = parse_args()
    directory = args.blueprint_dir.resolve()
    repo_root = args.repo_root.resolve()
    if args.baseline:
        if args.lock:
            raise SystemExit("--baseline and --lock are mutually exclusive")
        print(baseline_sha256(repo_root, directory))
        return 0
    errors = validate(directory, repo_root)
    if errors:
        print("Blueprint validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    if args.lock:
        try:
            lock(directory, repo_root)
        except (OSError, ValueError, subprocess.CalledProcessError) as error:
            print(f"Blueprint lock failed: {error}")
            return 1
        print(f"Blueprint approved contract locked: {directory / 'approval.json'}")
    else:
        print(f"Blueprint valid: {directory}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
