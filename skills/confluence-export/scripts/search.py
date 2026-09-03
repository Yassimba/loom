#!/usr/bin/env python3
"""Search Confluence Cloud without exposing the credentials stored by cme."""

from __future__ import annotations

import argparse
import base64
import json
import os
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import cast
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

JsonObject = dict[str, object]


def arguments() -> argparse.Namespace:
    """Read one text search and its optional Confluence scope."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("query", help="Text to find in Confluence pages.")
    parser.add_argument("--site", help="Configured Confluence base URL.")
    parser.add_argument("--space", help="Limit results to one space key.")
    parser.add_argument("--limit", type=int, default=25, choices=range(1, 101))
    return parser.parse_args()


def cme_config_path() -> Path:
    """Return the cme configuration path, with an override for automation."""
    if override := os.environ.get("CME_CONFIG_PATH"):
        return Path(override).expanduser()
    result = subprocess.run(
        ["cme", "config", "path"], check=True, capture_output=True, text=True
    )
    return Path(result.stdout.strip()).expanduser()


def object_value(value: object, name: str) -> Mapping[str, object]:
    """Require one JSON object from the cme configuration."""
    if not isinstance(value, dict):
        raise ValueError(f"cme configuration has no {name}")
    return cast("Mapping[str, object]", value)


def configured_access(config: Mapping[str, object], requested_site: str | None) -> tuple[str, str]:
    """Return the selected site and an HTTP Authorization value."""
    auth = object_value(config.get("auth"), "auth section")
    sites = object_value(auth.get("confluence"), "Confluence credentials")
    if requested_site is None:
        if len(sites) != 1:
            raise ValueError("select one configured instance with --site")
        site = next(iter(sites))
    else:
        site = requested_site.rstrip("/")
    credentials = object_value(sites.get(site), f"credentials for {site}")

    username = credentials.get("username")
    api_token = credentials.get("api_token")
    if isinstance(username, str) and username and isinstance(api_token, str) and api_token:
        encoded = base64.b64encode(f"{username}:{api_token}".encode()).decode()
        return site, f"Basic {encoded}"

    pat = credentials.get("pat")
    if isinstance(pat, str) and pat:
        return site, f"Bearer {pat}"
    raise ValueError(f"cme has no usable credentials for {site}")


def cql_text(value: str) -> str:
    """Quote user text as one CQL string value."""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def search_url(site: str, query: str, space: str | None, limit: int) -> str:
    """Build the official Confluence content-search request."""
    clauses = ["type=page", f"text~{cql_text(query)}"]
    if space:
        clauses.append(f"space={cql_text(space)}")
    parameters = urlencode(
        {"cql": " AND ".join(clauses), "limit": limit, "expand": "space,version"}
    )
    return f"{site}/wiki/rest/api/content/search?{parameters}"


def result_rows(site: str, payload: Mapping[str, object]) -> list[JsonObject]:
    """Return stable search fields and omit all authentication data."""
    raw_results = payload.get("results")
    if not isinstance(raw_results, list):
        return []
    rows: list[JsonObject] = []
    for raw in raw_results:
        if not isinstance(raw, dict):
            continue
        item = cast("Mapping[str, object]", raw)
        space = item.get("space")
        version = item.get("version")
        links = item.get("_links")
        webui = links.get("webui") if isinstance(links, dict) else None
        if isinstance(webui, str) and webui.startswith("/"):
            page_url: str | None = f"{site}/wiki{webui}"
        else:
            page_url = webui if isinstance(webui, str) else None
        rows.append(
            {
                "id": item.get("id"),
                "title": item.get("title"),
                "space": space.get("key") if isinstance(space, dict) else None,
                "updated": version.get("when") if isinstance(version, dict) else None,
                "url": page_url,
            }
        )
    return rows


def main() -> int:
    """Search the configured Confluence instance and print JSON results."""
    options = arguments()
    try:
        config = object_value(json.loads(cme_config_path().read_text()), "root object")
        site, authorization = configured_access(config, options.site)
        request = Request(
            search_url(site, options.query, options.space, options.limit),
            headers={"Authorization": authorization, "Accept": "application/json"},
        )
        with urlopen(request, timeout=30) as response:
            payload = object_value(json.load(response), "search response")
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError) as error:
        print(f"Confluence search setup failed: {error}", file=sys.stderr)
        return 2
    except HTTPError as error:
        print(f"Confluence search failed: HTTP {error.code} {error.reason}", file=sys.stderr)
        return 1
    except URLError as error:
        print(f"Confluence search failed: {error.reason}", file=sys.stderr)
        return 1

    print(json.dumps(result_rows(site, payload), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
