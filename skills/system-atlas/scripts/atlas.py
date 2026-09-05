#!/usr/bin/env python3
"""Search, inspect, and incrementally refresh a System Atlas using only the stdlib.

Run --help for commands. Semantic updates are made by the skill in a prepared
copy; publish validates that copy before replacing the original directory.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path


def read(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path: Path, value) -> None:
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def inside(root: Path, relative: str) -> Path:
    path = (root / relative).resolve()
    if Path(relative).is_absolute() or not path.is_relative_to(root.resolve()):
        raise ValueError(f"path leaves atlas: {relative}")
    return path


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE).decode()


def repositories(root: Path) -> dict:
    rows = read(root / "atlas.json").get("repositories", [])
    if not rows:
        raise ValueError("atlas has no verified repository pins; establish a baseline first")
    result = {}
    for row in rows:
        if row["id"] in result or not re.fullmatch(r"[0-9a-f]{40,64}", row["commit"]):
            raise ValueError("repository IDs must be unique and commits must be full object IDs")
        result[row["id"]] = row
    return result


def topics(root: Path) -> dict:
    result = {}
    for path in sorted((root / "topics").glob("*.json")):
        topic = read(path)
        if not re.fullmatch(r"[a-z0-9][a-z0-9._-]*", topic["id"]) or topic["id"] in result:
            raise ValueError(f"invalid or duplicate topic ID: {topic['id']}")
        result[topic["id"]] = topic
    if not result:
        raise ValueError("atlas has no topic records; establish searchable coverage first")
    return result


def elements(spec: dict) -> list:
    """Return semantic handles, never coordinates or SVG."""
    return [
        {"collection": collection, **{key: item[key] for key in
         ("id", "repo", "label", "sublabel", "from", "to", "code") if key in item}}
        for collection in ("nodes", "edges", "zones", "primitives")
        for item in spec.get(collection, []) if item.get("id")
    ]


def search_index(root: Path) -> list:
    rows = []
    for topic in topics(root).values():
        labels = []
        for figure in topic.get("figures", []):
            labels.extend(elements(read(inside(root, figure["json"]))))
        rows.append({"id": topic["id"], "title": topic["title"],
                     "summary": topic["summary"], "section": topic["section"],
                     "text": json.dumps([topic, labels], ensure_ascii=False).lower()})
    return rows


def search(root: Path, query: str, limit: int) -> list:
    terms = set(re.findall(r"[\w./:-]+", query.lower()))
    rows = read(root / "search.json") if (root / "search.json").exists() else search_index(root)
    # ponytail: linear search over the small topic index; use FTS if measured latency warrants it.
    scored = [(sum(term in row["text"] for term in terms), row) for row in rows]
    return [{key: value for key, value in row.items() if key != "text"}
            for score, row in sorted(scored, key=lambda pair: (-pair[0], pair[1]["id"]))
            if score][:limit]


def show(root: Path, topic_id: str, offset: int, limit: int) -> dict:
    topic = topics(root)[topic_id]
    facts = topic.get("facts", [])
    result = {key: topic[key] for key in ("id", "title", "summary", "section")}
    result["facts"] = facts[offset:offset + limit]
    source_ids = {source for fact in result["facts"] for source in fact["sources"]}
    result["sources"] = [source for source in topic["sources"] if source["id"] in source_ids]
    for field in ("dependsOn", "dependencies", "unknowns"):
        result[field] = topic.get(field, [])[offset:offset + limit]
    repos = repositories(root)
    result["repositories"] = {source["repo"]: repos[source["repo"]]
                              for source in result["sources"] + result["dependencies"]}
    count = max(len(topic.get(field, [])) for field in ("facts", "figures", "dependsOn", "dependencies", "unknowns"))
    result["nextOffset"] = offset + limit if offset + limit < count else None
    result["figures"] = [{"id": figure["id"], "question": figure["question"]}
                         for figure in topic.get("figures", [])[offset:offset + limit]]
    result["figureCount"] = len(topic.get("figures", []))
    result["nextFigureOffset"] = offset + limit if offset + limit < result["figureCount"] else None
    return result


def show_figure(root: Path, figure_id: str, offset: int, limit: int) -> dict:
    for topic in topics(root).values():
        for figure in topic.get("figures", []):
            if figure["id"] == figure_id:
                handles = elements(read(inside(root, figure["json"])))
                return {**figure, "elements": handles[offset:offset + limit],
                        "nextOffset": offset + limit if offset + limit < len(handles) else None}
    raise ValueError(f"unknown figure ID: {figure_id}")


def catalogue_types() -> set:
    catalogue = Path(__file__).resolve().parents[2] / "diagram-design/SKILL.md"
    return {line.split("|")[2].strip().strip("*") for line in catalogue.read_text().splitlines()
            if "](references/type-" in line and line.startswith("|") and line.split("|")[2].strip().startswith("**")}


def validate_coverage(manifest: dict) -> None:
    decisions = manifest.get("typeDecisions", [])
    if {row["type"] for row in decisions} != catalogue_types():
        raise ValueError(f"{manifest['section']}: consider every catalogue type")
    if any(not (row.get("subject") or row.get("reason")) for row in decisions):
        raise ValueError("each type decision needs a subject or reason")
    if not manifest.get("coverage") or not manifest.get("depthCheck"):
        raise ValueError("section needs coverage and depthCheck")
    if len(manifest["diagrams"]) < 12 and not manifest.get("quotaReason"):
        raise ValueError("fewer than 12 figures requires a coverage check and quotaReason")


def validate(root: Path) -> None:
    repos, records = repositories(root), topics(root)
    sections = {}
    figures = {}
    for path in (root / "diagrams").glob("*/manifest.json"):
        manifest = read(path)
        validate_coverage(manifest)
        sections[manifest["section"]] = manifest
        for figure in manifest["diagrams"]:
            figure_id = figure["id"]
            if figure_id in figures:
                raise ValueError(f"duplicate figure ID: {figure_id}")
            figures[figure_id] = str(path.parent.relative_to(root) / figure["json"])
            for field in ("file", "json"):
                if not inside(root, str(path.parent.relative_to(root) / figure[field])).is_file():
                    raise ValueError(f"missing figure {figure[field]}")
    for topic in records.values():
        if not topic.get("summary") or not topic.get("facts") or topic["section"] not in sections:
            raise ValueError(f"topic needs summary, facts, and a real section: {topic['id']}")
        for related in topic.get("dependsOn", []):
            if related not in records:
                raise ValueError(f"unknown topic dependency: {related}")
        for source in topic.get("sources", []):
            repo = repos[source["repo"]]
            path = source["path"]
            if Path(path).is_absolute() or ".." in Path(path).parts:
                raise ValueError(f"invalid source path: {path}")
            lines = git((root / repo["path"]).resolve(), "show", f"{repo['commit']}:{path}").splitlines()
            start, end = source["start"], source["end"]
            if not 1 <= start <= end <= len(lines) or source["anchor"] not in lines[start - 1:end]:
                raise ValueError(f"stale source anchor: {topic['id']} {path}:{start}-{end}")
        source_ids = {source["id"] for source in topic.get("sources", [])}
        if len(source_ids) != len(topic.get("sources", [])):
            raise ValueError(f"duplicate source IDs: {topic['id']}")
        for fact in topic["facts"]:
            if not fact.get("text") or not fact.get("sources") or not set(fact["sources"]) <= source_ids:
                raise ValueError(f"fact lacks source evidence: {topic['id']}")
        for figure in topic.get("figures", []):
            if figures.get(figure["id"]) != figure["json"]:
                raise ValueError(f"topic figure does not match manifest: {figure['id']}")
            spec = read(inside(root, figure["json"]))
            handles = [item["id"] for item in elements(spec)]
            if len(handles) != len(set(handles)):
                raise ValueError(f"duplicate element IDs: {figure['json']}")
    validate_bindings(root, records)


def validate_bindings(root: Path, records: dict) -> None:
    for manifest_path in (root / "diagrams").glob("*/manifest.json"):
        for figure in read(manifest_path)["diagrams"]:
            sources = [source for topic in records.values()
                       if any(item["id"] == figure["id"] for item in topic.get("figures", []))
                       for source in topic["sources"]]
            verified = {(source["repo"], f"{source['path']}:{source['start']}-{source['end']}")
                        for source in sources}
            spec = read(inside(root, str(manifest_path.parent.relative_to(root) / figure["json"])))
            expected = {}
            for collection in ("nodes", "edges", "zones", "primitives"):
                for item in spec.get(collection, []):
                    bindings = item.get("code", [])
                    bindings = [bindings] if isinstance(bindings, str) else bindings
                    for binding in bindings:
                        if not item.get("id") or (item.get("repo", figure.get("repo")), binding) not in verified:
                            raise ValueError(f"unverified figure binding: {figure['id']} {binding}")
                    if bindings:
                        expected[item["id"]] = (item.get("repo", figure.get("repo")), tuple(bindings))
            markup = inside(root, str(manifest_path.parent.relative_to(root) / figure["file"])).read_text()
            match = re.search(r"<svg\b.*?</svg>", markup, re.S)
            if not match:
                raise ValueError(f"missing SVG: {figure['file']}")
            try:
                svg = ET.fromstring(match.group())
            except ET.ParseError as error:
                raise ValueError(f"invalid SVG: {figure['file']}") from error
            rendered = {}
            for item in svg.iter():
                if "data-code" not in item.attrib:
                    continue
                element_id = item.attrib.get("data-element-id")
                value = (item.attrib.get("data-repo", figure.get("repo")),
                         tuple(binding.strip() for binding in item.attrib["data-code"].split(",")))
                if element_id in rendered and rendered[element_id] != value:
                    raise ValueError(f"conflicting rendered bindings: {figure['id']} {element_id}")
                rendered[element_id] = value
            if rendered != expected:
                raise ValueError(f"rendered bindings differ from JSON: {figure['id']}")


def changed_paths(repo: Path, base: str, target: str) -> list:
    tokens = git(repo, "diff", "--name-status", "-z", "--find-renames", base, target, "--").split("\0")
    changes = []
    index = 0
    while index < len(tokens) and tokens[index]:
        status, path = tokens[index:index + 2]
        index += 2
        row = {"status": status, "path": path}
        if status.startswith(("R", "C")):
            row["oldPath"], row["path"] = path, tokens[index]
            index += 1
        changes.append(row)
    return changes


def affected(root: Path, targets: dict) -> dict:
    repos, records = repositories(root), topics(root)
    if set(targets) - repos.keys():
        raise ValueError("target names an unknown repository")
    changes, commits, affected_ids = [], {}, set()
    for repo_id, repo in repos.items():
        path = (root / repo["path"]).resolve()
        target = git(path, "rev-parse", "--verify", f"{targets.get(repo_id, 'HEAD')}^{{commit}}").strip()
        commits[repo_id] = target
        for change in changed_paths(path, repo["commit"], target):
            paths = {change["path"], change.get("oldPath", change["path"])}
            direct = [topic["id"] for topic in records.values() if any(
                source["repo"] == repo_id and source["path"] in paths
                for source in topic.get("sources", []) + topic.get("dependencies", []))]
            affected_ids.update(direct)
            changes.append({"repo": repo_id, **change, "topics": direct})
    while True:
        dependents = {topic["id"] for topic in records.values()
                      if affected_ids.intersection(topic.get("dependsOn", []))}
        if dependents <= affected_ids:
            break
        affected_ids.update(dependents)
    return {"base": {key: row["commit"] for key, row in repos.items()}, "targets": commits,
            "topics": sorted(affected_ids), "changes": changes,
            "unmapped": [row for row in changes if not row["topics"]]}


def tree_hash(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"atlas must contain materialized files: {path}")
        if path.is_file():
            digest.update(str(path.relative_to(root)).encode() + b"\0" + path.read_bytes() + b"\0")
    return digest.hexdigest()


def prepare(root: Path, targets: dict) -> dict:
    root = root.resolve()
    delta = affected(root, targets)
    if not delta["changes"] and delta["base"] == delta["targets"]:
        return {"status": "unchanged"}
    before = tree_hash(root)
    stage = Path(tempfile.mkdtemp(prefix=f".{root.name}-refresh-", dir=root.parent))
    shutil.copytree(root, stage, dirs_exist_ok=True)
    config = read(stage / "atlas.json")
    for repo in config["repositories"]:
        repo["commit"] = delta["targets"][repo["id"]]
    write(stage / "atlas.json", config)
    write(stage / "refresh.json", {"destination": str(root), "originalSha256": before,
                                  **delta, "reviewed": False})
    return {"stage": str(stage), **delta}


def publish(stage: Path) -> dict:
    stage = stage.resolve()
    record = read(stage / "refresh.json")
    root = Path(record["destination"]).resolve()
    if stage.parent != root.parent or stage == root or not stage.name.startswith(f".{root.name}-refresh-"):
        raise ValueError("publish requires a sibling directory made by prepare")
    if record.get("reviewed") is not True:
        raise ValueError("review all changed/unmapped files and coverage before setting reviewed=true")
    reviewed = {(row["repo"], row["path"]) for row in record.get("decisions", []) if row.get("reason")}
    if any((row["repo"], row["path"]) not in reviewed for row in record["changes"]):
        raise ValueError("each changed/unmapped path needs a decision with repo, path, and reason")
    if tree_hash(root) != record["originalSha256"]:
        raise ValueError("published atlas changed since prepare; prepare again")
    if {key: row["commit"] for key, row in repositories(stage).items()} != record["targets"]:
        raise ValueError("staged pins differ from captured targets")
    tree_hash(stage)  # Refuse symlinks before reading or publishing staged content.
    validate(stage)
    write(stage / "search.json", search_index(stage))
    subprocess.run([sys.executable, str(Path(__file__).with_name("assemble.py")), str(stage)], check=True,
                   stdout=subprocess.PIPE)
    if tree_hash(root) != record["originalSha256"]:
        raise ValueError("published atlas changed during validation; prepare again")
    backup = Path(tempfile.mkdtemp(prefix=f".{root.name}-previous-", dir=root.parent))
    backup.rmdir()  # Empty reservation, never an existing atlas.
    root.rename(backup)
    try:
        stage.rename(root)
    except OSError:
        backup.rename(root)
        raise
    return {"published": str(root), "previous": str(backup)}


def orient(root: Path, targets: dict, output: Path) -> dict:
    """Write one map file for a change consumer plus one diff file per mapped path.
    The map lists affected topics with facts, sources and figure handles, and every
    changed path with its topics and diff file. One read replaces many calls."""
    delta = affected(root, targets)
    repos, records = repositories(root), topics(root)
    output.mkdir(parents=True, exist_ok=True)
    (output / "diffs").mkdir(exist_ok=True)
    lines = [f"# Orientation: {root.name}", "", "Base pins: " + json.dumps(delta["base"]),
             "Targets: " + json.dumps(delta["targets"]), "", f"## Affected topics ({len(delta['topics'])})"]
    for topic_id in delta["topics"]:
        topic = records[topic_id]
        lines += ["", f"### {topic_id}: {topic['title']}", topic["summary"]]
        for source in topic.get("sources", []):
            lines.append(f"- src {source['id']}: {source['path']}:{source['start']}-{source['end']}")
        for fact in topic["facts"]:
            lines.append(f"- {fact['text']} [{', '.join(fact['sources'])}]")
        for unknown in topic.get("unknowns", []):
            lines.append(f"- unknown: {unknown}")
        for figure in topic.get("figures", []):
            handles = elements(read(inside(root, figure["json"])))
            lines.append(f"- figure {figure['id']} ({figure['json']}): " + "; ".join(
                f"{h['id']}={h.get('label', h.get('from', '') + '>' + h.get('to', ''))}"
                + (f" @{','.join(h['code']) if isinstance(h['code'], list) else h['code']}" if h.get("code") else "")
                for h in handles))
    lines += ["", f"## Changed paths ({len(delta['changes'])}); mapped ones have a diff file", ""]
    for change in delta["changes"]:
        row = f"- {change['status']} {change['path']} -> {', '.join(change['topics']) or 'unmapped'}"
        if change["topics"]:
            repo = repos[change["repo"]]
            diff = git((root / repo["path"]).resolve(), "diff", repo["commit"],
                       delta["targets"][change["repo"]], "--", change["path"])
            diff_path = output / "diffs" / (change["path"].replace("/", "__") + ".diff")
            diff_path.write_text(diff, encoding="utf-8")
            row += f" ({diff_path.relative_to(output)}, {len(diff.splitlines())} lines)"
        lines.append(row)
    (output / "orientation.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return {"orientation": str(output / "orientation.md"), "topics": len(delta["topics"]),
            "changes": len(delta["changes"]), "unmapped": len(delta["unmapped"]),
            "bytes": (output / "orientation.md").stat().st_size}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("index", "validate", "search", "show", "figure", "affected", "prepare", "publish", "freeze", "orient"):
        command = commands.add_parser(name)
        command.add_argument("root", type=Path)
        if name == "search":
            command.add_argument("query")
            command.add_argument("--limit", type=int, default=5)
        if name in {"show", "figure"}:
            command.add_argument("id")
            command.add_argument("--offset", type=int, default=0)
            command.add_argument("--limit", type=int, default=10)
        if name in {"affected", "prepare", "orient"}:
            command.add_argument("--target", action="append", default=[], metavar="REPO=REV")
        if name == "orient":
            command.add_argument("--output", type=Path, required=True, help="directory")
        if name == "freeze":
            command.add_argument("ids", nargs="+")
            command.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        if getattr(args, "limit", 1) < 1 or getattr(args, "offset", 0) < 0:
            raise ValueError("limit must be positive; offset must be nonnegative")
        if args.command in {"index", "validate"}:
            validate(root)
            if args.command == "index":
                write(root / "search.json", search_index(root))
            result = {"status": "valid"}
        elif args.command == "search":
            result = search(root, args.query, args.limit)
        elif args.command == "show":
            result = show(root, args.id, args.offset, args.limit)
        elif args.command == "figure":
            result = show_figure(root, args.id, args.offset, args.limit)
        elif args.command in {"affected", "prepare", "orient"}:
            targets = dict(value.split("=", 1) for value in args.target)
            if args.command == "orient":
                result = orient(root, targets, args.output)
            else:
                result = (affected if args.command == "affected" else prepare)(root, targets)
        elif args.command == "freeze":
            result = {"repositories": repositories(root),
                      "topics": [topics(root)[topic_id] for topic_id in args.ids]}
            write(args.output, result)
            result = {"snapshot": str(args.output)}
        else:
            result = publish(root)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print(f"atlas: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
