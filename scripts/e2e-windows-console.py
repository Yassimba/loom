import argparse
import re
import threading
import time
from pathlib import Path

from winpty import PtyProcess

ANSI = re.compile(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\)|[()][A-Z0-9])")


def drive(command: list[str], keys: list[tuple[float, str]], timeout: float = 20) -> str:
    process = PtyProcess.spawn(command, dimensions=(40, 120))
    chunks: list[str] = []

    def read_output() -> None:
        while True:
            try:
                chunks.append(process.read(4096))
            except EOFError:
                return

    reader = threading.Thread(target=read_output, daemon=True)
    reader.start()
    for delay, keys_to_send in keys:
        time.sleep(delay)
        process.write(keys_to_send)
    deadline = time.monotonic() + timeout
    while process.isalive() and time.monotonic() < deadline:
        time.sleep(0.1)
    if process.isalive():
        process.close(force=True)
        raise TimeoutError(f"command did not exit: {command}")
    reader.join(timeout=2)
    return ANSI.sub("", "".join(chunks)).replace("\x00", "")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("loom")
    parser.add_argument("evidence", type=Path)
    args = parser.parse_args()
    args.evidence.mkdir(parents=True, exist_ok=True)

    keep_windows = drive(
        [args.loom, "setup", "--dry-run"],
        [(1, "n\r"), (2, "/chat\r"), (2, "\x1bq")],
    )
    (args.evidence / "native-keep-windows.txt").write_text(keep_windows, encoding="utf-8")
    if "Use WSL2 for the complete Loom setup?" not in keep_windows:
        raise AssertionError("WSL choice was not shown")
    if "No matches. Backspace widens, esc cancels." not in keep_windows:
        raise AssertionError("pi-chat remained searchable after choosing native Windows")

    use_wsl = drive([args.loom, "setup", "--dry-run"], [(1, "y\r")])
    (args.evidence / "native-use-wsl-dry-run.txt").write_text(use_wsl, encoding="utf-8")
    if "Would prepare WSL2; no changes made." not in use_wsl:
        raise AssertionError("WSL dry-run did not stop before mutation")


if __name__ == "__main__":
    main()
