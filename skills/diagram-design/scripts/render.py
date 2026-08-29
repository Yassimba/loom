#!/usr/bin/env python3
"""Expand a compact box-and-arrow JSON spec into Diagram Design HTML."""

from __future__ import annotations

import argparse
import html
import json
import math
import re
import sys
from pathlib import Path

SKILL_DIR = Path(__file__).resolve().parent.parent
TEMPLATE = SKILL_DIR / "assets" / "template.html"
SLUG_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
COLOR_RE = re.compile(r"^(?:#[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3})?|rgba?\([0-9.,% ]+\))$")
ROLES = {
    "focal",
    "backend",
    "step",
    "store",
    "external",
    "input",
    "optional",
    "security",
}
TONES = {"default", "accent", "link"}
PORTS = {"left", "right", "top", "bottom"}
SUPPORTED_TYPES = {
    "architecture",
    "it-state",
    "flowchart",
    "sequence",
    "state",
    "er",
    "timeline",
    "swimlane",
    "quadrant",
    "radar",
    "polar",
    "loop",
    "nested",
    "tree",
    "org-chart",
    "layers",
    "venn",
    "pyramid",
    "bar",
    "treemap",
    "line",
    "gantt",
    "scatter",
    "high-level",
    "process",
    "medallion",
    "data-flow",
    "dp-integration",
    "dp-security-matrix",
    "sankey",
    "fishbone",
    "wardley",
    "kanban",
    "journey",
    "deployment",
    "dependency",
    "uml-class",
    "story-map",
    "db-schema",
}
DEFAULT_TOKENS = {
    "paper": "#f5f5f5",
    "ink": "#2d3142",
    "muted": "#4f5d75",
    "soft": "#7a8399",
    "accent": "#eb6c36",
    "accent-tint": "rgba(235,108,54,0.08)",
    "link": "#2e5aa8",
    "rule": "rgba(45,49,66,0.12)",
}


def text(value: object) -> str:
    return html.escape(str(value), quote=True)


def number(value: object, name: str) -> float:
    if (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not math.isfinite(value)
    ):
        raise ValueError(f"{name} must be a finite number")
    return float(value)


def fmt(value: float) -> str:
    return str(int(value)) if value.is_integer() else f"{value:g}"


def tokens(spec: dict) -> dict[str, str]:
    result = dict(DEFAULT_TOKENS)
    overrides = spec.get("tokens", {})
    if not isinstance(overrides, dict):
        raise ValueError("tokens must be an object")
    for name, value in overrides.items():
        if (
            name not in result
            or not isinstance(value, str)
            or not COLOR_RE.fullmatch(value)
        ):
            raise ValueError(f"invalid token override {name!r}")
        result[name] = value
    return result


def endpoint(
    raw: object, offset: object, nodes: dict[str, dict]
) -> tuple[float, float]:
    if not isinstance(raw, str) or ":" not in raw:
        raise ValueError("edge endpoints use node-id:left|right|top|bottom")
    node_id, port = raw.rsplit(":", 1)
    if node_id not in nodes or port not in PORTS:
        raise ValueError(f"unknown edge endpoint {raw!r}")
    ratio = number(offset, "port offset")
    if not 0 <= ratio <= 1:
        raise ValueError("port offset must be between 0 and 1")
    node = nodes[node_id]
    x, y, width, height = (node[key] for key in ("x", "y", "width", "height"))
    if port == "left":
        return x, y + height * ratio
    if port == "right":
        return x + width, y + height * ratio
    if port == "top":
        return x + width * ratio, y
    return x + width * ratio, y + height


def line_command(start: tuple[float, float], end: tuple[float, float]) -> str:
    if start[1] == end[1]:
        return f"H {fmt(end[0])}"
    if start[0] == end[0]:
        return f"V {fmt(end[1])}"
    raise ValueError(f"connector segment {start} → {end} is not orthogonal")


def rounded_path(points: list[tuple[float, float]]) -> str:
    for start, end in zip(points, points[1:]):
        line_command(start, end)
    simplified = [points[0]]
    for point in points[1:]:
        if point == simplified[-1]:
            raise ValueError("connector route contains a zero-length segment")
        if len(simplified) > 1:
            a, b = simplified[-2], simplified[-1]
            if (a[0] == b[0] == point[0]) or (a[1] == b[1] == point[1]):
                simplified[-1] = point
                continue
        simplified.append(point)

    parts = [f"M {fmt(simplified[0][0])} {fmt(simplified[0][1])}"]
    cursor = simplified[0]
    for previous, corner, following in zip(simplified, simplified[1:], simplified[2:]):
        incoming = (
            math.copysign(1, corner[0] - previous[0])
            if corner[0] != previous[0]
            else 0,
            math.copysign(1, corner[1] - previous[1])
            if corner[1] != previous[1]
            else 0,
        )
        outgoing = (
            math.copysign(1, following[0] - corner[0])
            if following[0] != corner[0]
            else 0,
            math.copysign(1, following[1] - corner[1])
            if following[1] != corner[1]
            else 0,
        )
        before_length = abs(corner[0] - previous[0]) + abs(corner[1] - previous[1])
        after_length = abs(following[0] - corner[0]) + abs(following[1] - corner[1])
        radius = min(8.0, before_length / 2, after_length / 2)
        before = (corner[0] - incoming[0] * radius, corner[1] - incoming[1] * radius)
        after = (corner[0] + outgoing[0] * radius, corner[1] + outgoing[1] * radius)
        parts.append(line_command(cursor, before))
        parts.append(
            f"Q {fmt(corner[0])} {fmt(corner[1])} {fmt(after[0])} {fmt(after[1])}"
        )
        cursor = after
    parts.append(line_command(cursor, simplified[-1]))
    return " ".join(parts)


def tint(color: str, percent: int) -> str:
    return f"color-mix(in srgb, {color} {percent}%, transparent)"


def node_style(role: str, palette: dict[str, str]) -> tuple[str, str, str]:
    styles = {
        "focal": (palette["accent-tint"], palette["accent"], ""),
        "backend": (palette["paper"], palette["ink"], ""),
        "step": (palette["paper"], palette["ink"], ""),
        "store": (tint(palette["ink"], 5), palette["muted"], ""),
        "external": (tint(palette["ink"], 3), tint(palette["ink"], 30), ""),
        "input": (tint(palette["muted"], 10), palette["soft"], ""),
        "optional": (
            tint(palette["ink"], 2),
            tint(palette["ink"], 20),
            ' stroke-dasharray="4,3"',
        ),
        "security": (
            tint(palette["accent"], 5),
            tint(palette["accent"], 50),
            ' stroke-dasharray="4,4"',
        ),
    }
    return styles[role]


def paint(value: object, palette: dict[str, str], default: str) -> str:
    if value is None:
        return default
    if isinstance(value, str) and value in palette:
        return palette[value]
    if isinstance(value, str) and value in {"none", "transparent", "currentColor"}:
        return str(value)
    if isinstance(value, str) and "@" in value:
        name, raw_percent = value.rsplit("@", 1)
        if name in palette and raw_percent.isdigit() and 0 <= int(raw_percent) <= 100:
            return tint(palette[name], int(raw_percent))
    if isinstance(value, str) and COLOR_RE.fullmatch(value):
        return value
    raise ValueError(f"invalid paint {value!r}")


def primitive_style(item: dict, palette: dict[str, str]) -> str:
    fill = paint(item.get("fill"), palette, "none")
    stroke = paint(item.get("stroke"), palette, "none")
    width = number(item.get("strokeWidth", 1), "primitive strokeWidth")
    attributes = [f'fill="{fill}"', f'stroke="{stroke}"', f'stroke-width="{fmt(width)}"']
    if "opacity" in item:
        opacity = number(item["opacity"], "primitive opacity")
        if not 0 <= opacity <= 1:
            raise ValueError("primitive opacity must be between 0 and 1")
        attributes.append(f'opacity="{fmt(opacity)}"')
    dash = item.get("dash")
    if dash is not None:
        if not isinstance(dash, str) or not re.fullmatch(r"[0-9., ]+", dash):
            raise ValueError("primitive dash must contain only numbers and separators")
        attributes.append(f'stroke-dasharray="{dash}"')
    marker = item.get("marker")
    if marker is not None:
        if marker not in TONES:
            raise ValueError(f"unknown primitive marker {marker!r}")
        marker_id = {"default": "arrow", "accent": "arrow-accent", "link": "arrow-link"}[marker]
        attributes.append(f'marker-end="url(#{marker_id})"')
    linecap = item.get("linecap")
    if linecap is not None:
        if linecap not in {"butt", "round", "square"}:
            raise ValueError(f"invalid linecap {linecap!r}")
        attributes.append(f'stroke-linecap="{linecap}"')
    linejoin = item.get("linejoin")
    if linejoin is not None:
        if linejoin not in {"arcs", "bevel", "miter", "round"}:
            raise ValueError(f"invalid linejoin {linejoin!r}")
        attributes.append(f'stroke-linejoin="{linejoin}"')
    return " ".join(attributes)


def points(value: object, name: str) -> str:
    if not isinstance(value, list) or len(value) < 2:
        raise ValueError(f"{name} needs at least two points")
    parsed = []
    for point in value:
        if not isinstance(point, list) or len(point) != 2:
            raise ValueError(f"{name} points must be [x,y]")
        parsed.append(",".join(fmt(number(coordinate, f"{name} coordinate")) for coordinate in point))
    return " ".join(parsed)


def render_primitive(item: dict, palette: dict[str, str]) -> str:
    if not isinstance(item, dict):
        raise ValueError("each primitive must be an object")
    kind = item.get("kind")
    style = primitive_style(item, palette)
    if kind == "rect":
        x, y, width, height = (number(item.get(key), f"rect {key}") for key in ("x", "y", "width", "height"))
        radius = number(item.get("radius", 0), "rect radius")
        return f'<rect x="{fmt(x)}" y="{fmt(y)}" width="{fmt(width)}" height="{fmt(height)}" rx="{fmt(radius)}" {style}/>'
    if kind in {"circle", "ellipse"}:
        cx, cy = number(item.get("cx"), f"{kind} cx"), number(item.get("cy"), f"{kind} cy")
        if kind == "circle":
            radius = number(item.get("r"), "circle r")
            return f'<circle cx="{fmt(cx)}" cy="{fmt(cy)}" r="{fmt(radius)}" {style}/>'
        rx, ry = number(item.get("rx"), "ellipse rx"), number(item.get("ry"), "ellipse ry")
        return f'<ellipse cx="{fmt(cx)}" cy="{fmt(cy)}" rx="{fmt(rx)}" ry="{fmt(ry)}" {style}/>'
    if kind == "line":
        values = [number(item.get(key), f"line {key}") for key in ("x1", "y1", "x2", "y2")]
        return f'<line x1="{fmt(values[0])}" y1="{fmt(values[1])}" x2="{fmt(values[2])}" y2="{fmt(values[3])}" {style}/>'
    if kind in {"polyline", "polygon"}:
        return f'<{kind} points="{points(item.get("points"), kind)}" {style}/>'
    if kind == "path":
        path = item.get("d")
        if not isinstance(path, str) or not path.strip():
            raise ValueError("path d must be a non-empty string")
        return f'<path d="{text(path)}" {style}/>'
    if kind == "text":
        x, y = number(item.get("x"), "text x"), number(item.get("y"), "text y")
        family = item.get("font", "sans")
        families = {"sans": "'Geist', sans-serif", "mono": "'Geist Mono', monospace", "serif": "'Instrument Serif', serif"}
        if family not in families:
            raise ValueError(f"unknown text font {family!r}")
        anchor = item.get("anchor", "start")
        if anchor not in {"start", "middle", "end"}:
            raise ValueError(f"invalid text anchor {anchor!r}")
        size = number(item.get("size", 12), "text size")
        fill = paint(item.get("fill"), palette, palette["ink"])
        weight = text(item.get("weight", 400))
        extra = f' font-style="italic"' if item.get("italic") else ""
        if "letterSpacing" in item:
            extra += f' letter-spacing="{fmt(number(item["letterSpacing"], "text letterSpacing"))}"'
        if "rotate" in item:
            angle = number(item["rotate"], "text rotate")
            extra += f' transform="rotate({fmt(angle)} {fmt(x)} {fmt(y)})"'
        return f'<text x="{fmt(x)}" y="{fmt(y)}" fill="{fill}" font-size="{fmt(size)}" font-weight="{weight}" font-family="{families[family]}" text-anchor="{anchor}"{extra}>{text(item.get("text", ""))}</text>'
    raise ValueError(f"unknown primitive kind {kind!r}")


def render_zone(zone: dict, palette: dict[str, str]) -> str:
    if not isinstance(zone, dict):
        raise ValueError("each zone must be an object")
    x, y, width, height = (
        number(zone.get(key), f"zone {key}") for key in ("x", "y", "width", "height")
    )
    label = text(zone.get("label", ""))
    label_width = max(40, math.ceil((len(str(zone.get("label", ""))) * 5 + 16) / 4) * 4)
    return (
        f'<rect x="{fmt(x)}" y="{fmt(y)}" width="{fmt(width)}" height="{fmt(height)}" rx="8" '
        f'fill="{tint(palette["ink"], 2)}" stroke="{palette["rule"]}" stroke-width="0.8"/>\n'
        f'<rect x="{fmt(x + 12)}" y="{fmt(y + 4)}" width="{label_width}" height="12" rx="2" fill="{palette["paper"]}"/>\n'
        f'<text x="{fmt(x + 12 + label_width / 2)}" y="{fmt(y + 13)}" fill="{palette["soft"]}" font-size="8" '
        f'font-family="\'Geist Mono\', monospace" text-anchor="middle" letter-spacing="0.12em">{label}</text>'
    )


def render_node(node: dict, palette: dict[str, str]) -> str:
    x, y, width, height = (node[key] for key in ("x", "y", "width", "height"))
    fill, stroke, extra = node_style(node["role"], palette)
    tag = str(node.get("tag", ""))
    sublabel = str(node.get("sublabel", ""))
    center_x, center_y = x + width / 2, y + height / 2
    lines = [
        f'<rect x="{fmt(x)}" y="{fmt(y)}" width="{fmt(width)}" height="{fmt(height)}" rx="6" fill="{palette["paper"]}"/>',
        f'<rect x="{fmt(x)}" y="{fmt(y)}" width="{fmt(width)}" height="{fmt(height)}" rx="6" fill="{fill}" stroke="{stroke}" stroke-width="1"{extra}/>',
    ]
    if tag:
        tag_width = max(28, math.ceil((len(tag) * 5 + 12) / 4) * 4)
        lines.extend(
            [
                f'<rect x="{fmt(x + 8)}" y="{fmt(y + 8)}" width="{tag_width}" height="12" rx="2" fill="transparent" stroke="{stroke}" stroke-opacity="0.4" stroke-width="0.8"/>',
                f'<text x="{fmt(x + 8 + tag_width / 2)}" y="{fmt(y + 17)}" fill="{stroke}" font-size="8" font-family="\'Geist Mono\', monospace" text-anchor="middle">{text(tag.upper())}</text>',
            ]
        )
    name_y = center_y - 2 if sublabel else center_y + 4
    lines.append(
        f'<text x="{fmt(center_x)}" y="{fmt(name_y)}" fill="{palette["ink"]}" font-size="12" font-weight="600" font-family="\'Geist\', sans-serif" text-anchor="middle">{text(node["label"])}</text>'
    )
    if sublabel:
        lines.append(
            f'<text x="{fmt(center_x)}" y="{fmt(center_y + 16)}" fill="{palette["muted"]}" font-size="8" font-family="\'Geist Mono\', monospace" text-anchor="middle">{text(sublabel)}</text>'
        )
    return "\n".join(lines)


def render_edge(
    edge: dict, nodes: dict[str, dict], palette: dict[str, str]
) -> tuple[str, str]:
    if not isinstance(edge, dict):
        raise ValueError("each edge must be an object")
    start = endpoint(edge.get("from"), edge.get("fromOffset", 0.5), nodes)
    end = endpoint(edge.get("to"), edge.get("toOffset", 0.5), nodes)
    raw_via = edge.get("via", [])
    if not isinstance(raw_via, list) or any(
        not isinstance(point, list) or len(point) != 2 for point in raw_via
    ):
        raise ValueError("each via point must contain x and y")
    via = [
        tuple(number(value, "via coordinate") for value in point) for point in raw_via
    ]
    path = rounded_path([start, *via, end])
    tone = edge.get("tone", "default")
    if tone not in TONES:
        raise ValueError(f"unknown edge tone {tone!r}")
    color = {
        "default": palette["muted"],
        "accent": palette["accent"],
        "link": palette["link"],
    }[tone]
    marker = {"default": "arrow", "accent": "arrow-accent", "link": "arrow-link"}[tone]
    dash = ' stroke-dasharray="5,4"' if edge.get("dashed") else ""
    connector = f'<path d="{path}" fill="none" stroke="{color}" stroke-width="1.2" marker-end="url(#{marker})"{dash}/>'
    label = edge.get("label")
    if not label:
        return connector, ""
    if not isinstance(label, dict):
        raise ValueError("edge label must be an object")
    label_text = str(label.get("text", ""))
    x, y = number(label.get("x"), "label x"), number(label.get("y"), "label y")
    width = max(24, math.ceil((len(label_text) * 5 + 16) / 4) * 4)
    markup = (
        f'<rect x="{fmt(x - width / 2)}" y="{fmt(y - 10)}" width="{width}" height="12" rx="2" fill="{palette["paper"]}"/>\n'
        f'<text x="{fmt(x)}" y="{fmt(y)}" fill="{palette["soft"]}" font-size="8" font-family="\'Geist Mono\', monospace" text-anchor="middle" letter-spacing="0.06em">{text(label_text.upper())}</text>'
    )
    return connector, markup


def render_legend(items: object, palette: dict[str, str], view_box: list[float]) -> str:
    if not items:
        return ""
    if not isinstance(items, list) or len(items) > 5:
        raise ValueError("legend must be a list of at most five items")
    origin_x, origin_y, width, height = view_box
    y = origin_y + height - 40
    lines = [
        f'<line x1="{fmt(origin_x + 32)}" y1="{fmt(y - 16)}" x2="{fmt(origin_x + width - 32)}" y2="{fmt(y - 16)}" stroke="{palette["rule"]}" stroke-width="0.8"/>'
    ]
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            raise ValueError("each legend item must be an object")
        role = item.get("role", "backend")
        if role not in ROLES:
            raise ValueError(f"unknown legend role {role!r}")
        fill, stroke, extra = node_style(role, palette)
        x = origin_x + 40 + index * 180
        lines.extend(
            [
                f'<rect x="{fmt(x)}" y="{fmt(y - 8)}" width="12" height="12" rx="2" fill="{fill}" stroke="{stroke}" stroke-width="1"{extra}/>',
                f'<text x="{fmt(x + 20)}" y="{fmt(y + 2)}" fill="{palette["muted"]}" font-size="8" font-family="\'Geist Mono\', monospace">{text(item.get("label", role))}</text>',
            ]
        )
    return "\n".join(lines)


def normalize_nodes(raw_nodes: object) -> dict[str, dict]:
    if not isinstance(raw_nodes, list):
        raise ValueError("nodes must be a list")
    result = {}
    for raw in raw_nodes:
        if not isinstance(raw, dict):
            raise ValueError("each node must be an object")
        node_id = raw.get("id")
        role = raw.get("role", "backend")
        if (
            not isinstance(node_id, str)
            or not SLUG_RE.fullmatch(node_id)
            or node_id in result
        ):
            raise ValueError(f"invalid or duplicate node id {node_id!r}")
        if role not in ROLES:
            raise ValueError(f"unknown node role {role!r}")
        node = dict(raw, role=role)
        for key in ("x", "y", "width", "height"):
            node[key] = number(raw.get(key), f"node {node_id} {key}")
        if node["width"] < 80 or node["height"] < 48:
            raise ValueError(f"node {node_id} must be at least 80x48")
        if not str(raw.get("label", "")).strip():
            raise ValueError(f"node {node_id} needs a label")
        result[node_id] = node
    return result


def render(spec: dict) -> str:
    required = {"version", "type", "slug", "title", "description"}
    missing = required - spec.keys()
    if missing:
        raise ValueError(f"missing fields: {', '.join(sorted(missing))}")
    if spec["version"] != 1:
        raise ValueError("version must be 1")
    if spec["type"] not in SUPPORTED_TYPES:
        raise ValueError(f"type must be one of {', '.join(sorted(SUPPORTED_TYPES))}")
    if not str(spec["title"]).strip() or not str(spec["description"]).strip():
        raise ValueError("title and description must be non-empty")
    slug = spec["slug"]
    if not isinstance(slug, str) or not SLUG_RE.fullmatch(slug):
        raise ValueError("slug must be lowercase kebab-case")
    palette = tokens(spec)
    nodes = normalize_nodes(spec.get("nodes", []))
    edges = spec.get("edges", [])
    if not isinstance(edges, list):
        raise ValueError("edges must be a list")
    view_box = spec.get("viewBox", [0, 0, 1000, 600])
    if not isinstance(view_box, list) or len(view_box) != 4:
        raise ValueError("viewBox must contain four numbers")
    view_box = [number(value, "viewBox value") for value in view_box]
    if view_box[2] <= 0 or view_box[3] <= 0:
        raise ValueError("viewBox width and height must be positive")
    zones = spec.get("zones", [])
    primitives = spec.get("primitives", [])
    if not isinstance(zones, list):
        raise ValueError("zones must be a list")
    if not isinstance(primitives, list):
        raise ValueError("primitives must be a list")

    primitives_markup = "\n".join(
        render_primitive(item, palette) for item in primitives
    )
    zones_markup = "\n".join(render_zone(zone, palette) for zone in zones)
    rendered_edges = [render_edge(edge, nodes, palette) for edge in edges]
    connectors = "\n".join(item[0] for item in rendered_edges)
    labels = "\n".join(item[1] for item in rendered_edges if item[1])
    nodes_markup = "\n".join(render_node(node, palette) for node in nodes.values())
    legend_markup = render_legend(spec.get("legend", []), palette, view_box)
    body = "\n\n".join(
        part
        for part in (
            primitives_markup,
            zones_markup,
            connectors,
            labels,
            nodes_markup,
            legend_markup,
        )
        if part
    )

    template = TEMPLATE.read_text(encoding="utf-8")
    replacements = {
        "<title>Diagram</title>": f"<title>{text(spec['title'])}</title>",
        "[Type]": text(str(spec["type"]).replace("-", " ").title()),
        "[Diagram title]": text(spec["title"]),
        "[diagram-slug]": slug,
        "[One sentence describing what the diagram shows]": text(spec["description"]),
        'viewBox="0 0 1000 600"': f'viewBox="{" ".join(fmt(value) for value in view_box)}"',
        "      <!-- Draw arrows first, then nodes. Replace with your content. -->": "      "
        + body.replace("\n", "\n      "),
    }
    for old, new in replacements.items():
        template = template.replace(old, new)
    for name, default in DEFAULT_TOKENS.items():
        template = template.replace(default, palette[name])
    return template


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("spec", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        spec = json.loads(args.spec.read_text(encoding="utf-8"))
        output = render(spec)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8")
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
