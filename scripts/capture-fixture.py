#!/usr/bin/env python3
"""Write the capture fixture replaying tests/fixtures/wagers-v1.json.

Every portrait answers capture/avatar.jpg, a drawn placeholder,
so the shots show the photo path and no third-party photos get committed.
"""

import base64
import gzip
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NEXUS = "https://nexus.braiinsforge.com"
FEED = f"{NEXUS}/api/v1/data/kicomtracker/wagers"
# An hour after the feed's generated_at, so the closed bets are still inside the default retention.
START = "2026-11-22T11:04:32+01:00"


def main():
    feed = json.loads((ROOT / "tests/fixtures/wagers-v1.json").read_text())
    manifest = json.loads((ROOT / "manifest.json").read_text())
    avatars = sorted({NEXUS + person["avatar"]
                      for wager in feed["data"]["wagers"]
                      for person in [wager["kicom"], *wager["counterparties"]]
                      if person.get("avatar")})
    header = {
        "time": START,
        "initial_params": {key: param["default_value"] for key, param in manifest["params"].items()},
        "initial_system": {"settings": {"timezone": "Europe/Prague"}},
    }
    avatar = base64.b64encode((ROOT / "capture/avatar.jpg").read_bytes()).decode()
    rotation_ms = manifest["params"]["rotation_seconds"]["default_value"] * 1000
    events = [
        {"at_ms": 100, "type": "fetch", "method": "GET", "url": FEED, "status": 200,
         "headers": [["content-type", "application/json"]], "body": {"json": feed}},
        *({"at_ms": 200, "type": "fetch", "method": "GET", "url": url, "status": 200,
           "headers": [["content-type", "image/jpeg"]], "body": {"b64": avatar}}
          for url in avatars),
        {"at_ms": 1000, "type": "capture"},
        {"at_ms": 1000 + rotation_ms, "type": "capture"},
    ]
    lines = "".join(json.dumps(line, ensure_ascii=False) + "\n" for line in [header, *events])
    out = ROOT / "capture/fixtures/wagers-v1.jsonl.gz"
    out.parent.mkdir(parents=True, exist_ok=True)
    # mtime=0 keeps the file byte-identical across regenerations.
    out.write_bytes(gzip.compress(lines.encode(), mtime=0))


if __name__ == "__main__":
    main()
