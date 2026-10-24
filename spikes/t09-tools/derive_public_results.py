"""P00-T09: derive a commit-safe copy of a sanitised results.json (Opus-owned; deterministic, stdlib only).

Usage: python3 -I -B derive_public_results.py <results.json> <out.json>

Why: the run-time PEM fixture body is the C05 canary (first 64-character line) followed by further
random synthetic lines that are not canaries, so the harness guard does not detect them and they can
survive in persisted excerpts. This script replaces every 64-character base64-alphabet line of high
character entropy (> 4.5 bits/char) inside any `*_excerpt` string with a fixed label, and changes
nothing else. The original file is kept unchanged; its sha256 and the replacement count are written
to <out.json>.meta.
"""

from __future__ import annotations

import collections
import hashlib
import json
import math
import re
import sys
from pathlib import Path

LINE = re.compile(r"(?<![A-Za-z0-9+/])[A-Za-z0-9+/]{64}(?![A-Za-z0-9+/])")
LABEL = "<synthetic-pem-body-line-redacted>"


def entropy(s: str) -> float:
    c = collections.Counter(s)
    return -sum(v / len(s) * math.log2(v / len(s)) for v in c.values())


def main() -> int:
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    raw = src.read_bytes()
    d = json.loads(raw)
    count = 0

    def fix(s: str) -> str:
        nonlocal count

        def rep(m):
            nonlocal count
            if entropy(m.group(0)) > 4.5:
                count += 1
                return LABEL
            return m.group(0)
        return LINE.sub(rep, s)

    def walk(v, key=""):
        if isinstance(v, dict):
            return {k: walk(x, k) for k, x in v.items()}
        if isinstance(v, list):
            return [walk(x, key) for x in v]
        if isinstance(v, str) and key.endswith("_excerpt"):
            return fix(v)
        return v

    out = walk(d)
    dst.write_text(json.dumps(out, indent=1, ensure_ascii=False))
    Path(str(dst) + ".meta").write_text(json.dumps({
        "derived_from": str(src), "source_sha256": hashlib.sha256(raw).hexdigest(),
        "derived_sha256": hashlib.sha256(dst.read_bytes()).hexdigest(),
        "replaced_lines": count, "rule": "64-char base64-alphabet runs with entropy > 4.5 bits/char inside *_excerpt fields",
        "label": LABEL}, indent=1) + "\n")
    print(f"replaced {count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
