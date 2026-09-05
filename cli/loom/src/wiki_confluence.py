"""Private-stdin adapter for CME 5.4's URL-keyed config. Never emits input/errors."""
import copy
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import tempfile


def no_symlinks(path):
    for part in (path, *path.parents):
        if part.is_symlink():
            raise ValueError("unsafe path")
    if path.exists() and not path.is_file():
        raise ValueError("not a file")


def config_path():
    # This matches CME's get_app_config_path without its mkdir side effect.
    from typer import get_app_dir
    override = os.environ.get("CME_CONFIG_PATH")
    return Path(override) if override else Path(get_app_dir("confluence-markdown-exporter")) / "app_data.json"


def snapshot(path):
    no_symlinks(path)
    if not path.exists():
        return {}, None
    if path.stat().st_size > 4 * 1024 * 1024:
        raise ValueError("oversized config")
    raw = path.read_bytes()
    data = json.loads(raw)
    if not isinstance(data, dict):
        raise ValueError("invalid root")
    return data, hashlib.sha256(raw).hexdigest()


def apply(request):
    path = config_path().absolute()
    data, digest = snapshot(path)
    # Import only after rejecting unsafe paths. CME's module creates the parent.
    from confluence_markdown_exporter.utils.app_data_store import ConfigModel
    ConfigModel.model_validate(copy.deepcopy(data), strict=True)
    from urllib.parse import urlsplit
    url = request["url"].strip().rstrip("/")
    parsed = urlsplit(url)
    if (parsed.scheme not in ("https", "http") or not parsed.hostname
            or parsed.username or parsed.password or parsed.query or parsed.fragment
            or any(c.isspace() or ord(c) < 32 for c in url)):
        raise ValueError("invalid URL")
    auth = data.setdefault("auth", {})
    accounts = auth.setdefault("confluence", {})
    if not isinstance(accounts, dict) or any(not k.startswith(("http://", "https://")) for k in accounts):
        raise ValueError("legacy account layout")
    keys = [k for k in accounts if k.rstrip("/") == url]
    if len(keys) > 1:
        raise ValueError("ambiguous accounts")
    if request["action"] == "inspect":
        return {"exists": bool(keys), "digest": digest}
    if request["action"] != "save" or request.get("digest") != digest:
        raise ValueError("configuration changed")
    if keys and not request.get("replace", False):
        raise ValueError("replacement not approved")
    username = request["username"].strip()
    token = request["token"].strip()
    if not token or len(token) > 16384 or any(ord(c) < 32 for c in token + username):
        raise ValueError("invalid credentials")
    pat = request["pat"]
    if not isinstance(pat, bool) or (not pat and not username):
        raise ValueError("missing username")
    account = accounts.pop(keys[0]) if keys else {}
    account.update(username=username, api_token="" if pat else token,
                   pat=token if pat else "", session_cookies="")
    accounts[url] = account
    ConfigModel.model_validate(copy.deepcopy(data), strict=True)
    encoded = (json.dumps(data, indent=2) + "\n").encode()
    temporary = None
    try:
        fd, temporary = tempfile.mkstemp(prefix=".loom-cme-", dir=path.parent)
        with os.fdopen(fd, "wb") as stream:
            os.fchmod(stream.fileno(), stat.S_IRUSR | stat.S_IWUSR)
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        if snapshot(path)[1] != digest:
            raise ValueError("configuration changed")
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary:
            os.unlink(temporary)
    return {"saved": True}


if __name__ == "__main__":
    try:
        request = json.loads(sys.stdin.buffer.read(65537))
        print(json.dumps(apply(request)))
    except Exception:
        # Validation errors include rejected input; never serialize exceptions.
        print('{"error":"Cannot configure CME: invalid input/config, unsafe path, or concurrent change. Existing credentials were kept."}')
        sys.exit(1)
