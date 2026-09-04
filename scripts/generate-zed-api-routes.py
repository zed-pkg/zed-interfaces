#!/usr/bin/env python3
"""Generate typed route clients from route-maps/zed-api.route-map.json.

Uses github.com/oresoftware/api-docs `scripts/generate-routes.py` (HTTP / TCP /
WebSocket call frames). Looks for that checkout at $ZED_API_DOCS, then the
usual local codes paths.
"""

from __future__ import annotations

import argparse
import os
import runpy
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAP = ROOT / "route-maps" / "zed-api.route-map.json"
OUT = ROOT / "generated"


def api_docs_root() -> Path:
    candidates = []
    env = os.environ.get("ZED_API_DOCS")
    if env:
        candidates.append(Path(env))
    home = Path.home() / "codes"
    candidates.extend(
        [
            home / "ores" / "api-docs",
            home / "oresoftware" / "api-docs",
        ]
    )
    for path in candidates:
        script = path / "scripts" / "generate-routes.py"
        if script.is_file():
            return path
    raise SystemExit(
        "oresoftware/api-docs not found; set ZED_API_DOCS to that checkout"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not MAP.is_file():
        raise SystemExit(f"missing route map: {MAP}")
    docs = api_docs_root()
    script = docs / "scripts" / "generate-routes.py"
    argv = [
        str(script),
        "--map",
        str(MAP),
        "--out",
        str(OUT),
    ]
    if args.check:
        argv.append("--check")
    sys.argv = argv
    runpy.run_path(str(script), run_name="__main__")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
