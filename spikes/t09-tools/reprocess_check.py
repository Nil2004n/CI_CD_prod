"""P00-T09 H-3 follow-up: pass existing results through the corrected shared output path (Opus-owned).

Usage: python3 -I -B reprocess_check.py <out-summary.json> <label>=<results.json> [...]

Reads each results file (never modifies it) and, in memory only:
- applies the corrected `common.Redactor.text` to every string, using a FRESH canary set and fresh
  synthetic components (the original run-time values were never persisted and cannot be recovered),
  and counts strings it would change;
- counts residual indicators: `PWTFAKE` substrings and 64-character base64-alphabet runs with
  entropy > 4.5 bits/char (the H-3 PEM-body line shape);
- re-escapes every string with `common.md_escape` and counts control/bidi characters left.
Writes counts only — no content — to <out-summary.json>.
"""

from __future__ import annotations

import collections
import hashlib
import json
import math
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common  # noqa: E402
import fixtures  # noqa: E402

RUN64 = re.compile(r"[A-Za-z0-9+/]{64}")


def ent(s: str) -> float:
    c = collections.Counter(s)
    return -sum(v / len(s) * math.log2(v / len(s)) for v in c.values())


def strings(v):
    if isinstance(v, dict):
        for k, x in v.items():
            yield str(k)
            yield from strings(x)
    elif isinstance(v, list):
        for x in v:
            yield from strings(x)
    elif isinstance(v, str):
        yield v


def main() -> int:
    out = Path(sys.argv[1])
    canaries = fixtures.gen_canaries()
    red = common.Redactor(canaries, fixtures.canary_markers(canaries), fixtures.synthetic_components(canaries))
    summary = {"note": "fresh canaries/components: the original run-time fixture values are unavailable, so this cannot "
                       "retro-detect original fixture material; it checks idempotence and residual indicators only",
               "files": {}}
    for arg in sys.argv[2:]:
        label, path = arg.split("=", 1)
        raw = Path(path).read_bytes()
        d = json.loads(raw)
        ss = list(strings(d))
        text = raw.decode("utf-8", "replace")
        summary["files"][label] = {
            "sha256": hashlib.sha256(raw).hexdigest(),
            "strings": len(ss),
            "strings_changed_by_corrected_redactor": sum(1 for s in ss if red.text(s) != s),
            "pwtfake_substrings": text.count("PWTFAKE"),
            "high_entropy_64_runs": sum(1 for m in RUN64.finditer(text) if ent(m.group(0)) > 4.5),
            "control_or_bidi_after_md_escape": sum(1 for s in ss for ch in common.md_escape(s)
                                                   if ord(ch) < 32 or 0x7F <= ord(ch) <= 0x9F or ord(ch) in common._INVISIBLE),
        }
    out.write_text(json.dumps(summary, indent=1) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
