"""P00-T10 consistency check (Opus-owned; stdlib only, offline, read-only).

Usage: python3 -I -B check_consistency.py <repo-root>

Checks, printing one PASS/FAIL line each and exiting 1 on any FAIL:
- every frozen-interface function and status string (spikes/P00-T10-request.md section 2) appears in the
  prototype (spikes/t10-cache/cache.py) and in the plan (spikes/P00-T10-schema-cache-plan.md);
- both frozen regexes appear verbatim in the prototype;
- every [Ln] citation in the plan exists in evidence/P00/P00-T10/lookups.md, and the plan cites at least one;
- the plan names the FR-26 requirement, the cache_only network policy, and the schema_fetch operation.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

FUNCS = ["validate_identifiers", "schema_filename", "load_manifest", "verify_set", "lookup", "freshness"]
STATUSES = ["available", "unavailable_missing_manifest", "unavailable_bad_manifest", "unavailable_hash_mismatch",
            "unavailable_missing_file", "unavailable_extra_file", "unavailable_symlink", "unavailable_not_regular",
            "found", "unsupported_missing_schema", "invalid_identifier", "containment_violation", "set_unavailable"]
KIND_RE = r"^[A-Za-z][A-Za-z0-9]{0,62}$"
API_RE = r"^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*/)?v[0-9]+(?:(?:alpha|beta)[0-9]+)?$"


def main() -> int:
    root = Path(sys.argv[1])
    req = (root / "spikes/P00-T10-request.md").read_text()
    code = (root / "spikes/t10-cache/cache.py").read_text()
    plan = (root / "spikes/P00-T10-schema-cache-plan.md").read_text()
    lookups = (root / "evidence/P00/P00-T10/lookups.md").read_text()
    results = []

    def ck(name, ok, detail=""):
        results.append(ok)
        print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail and not ok else ""))

    for f in FUNCS:
        ck(f"request names {f}", f in req)
        ck(f"prototype defines {f}", re.search(rf"^def {f}\(", code, re.M) is not None)
        ck(f"plan names {f}", f in plan)
    for s in STATUSES:
        ck(f"request lists status {s}", f"`{s}`" in req)
        ck(f"prototype uses status {s}", f'"{s}"' in code or f"'{s}'" in code)
        ck(f"plan names status {s}", s in plan)
    # Compare the module's actual regex values (source may split a raw string across lines).
    sys.path.insert(0, str(root / "spikes/t10-cache"))
    import cache  # noqa: E402  (stdlib-only prototype; import has no side effects)
    ck("prototype KIND_RE equals frozen kind regex", getattr(cache, "KIND_RE", None) == KIND_RE)
    ck("prototype API_VERSION_RE equals frozen apiVersion regex", getattr(cache, "API_VERSION_RE", None) == API_RE)
    known = set(re.findall(r"^\| (L\d+) \|", lookups, re.M))
    cited = set(re.findall(r"\[(L\d+)", plan)) | set(re.findall(r"\bL(\d+)\b", " ".join(re.findall(r"\[[^\]]*L\d+[^\]]*\]", plan))))
    cited = {c if c.startswith("L") else "L" + c for c in cited}
    ck("plan cites at least one lookup", bool(cited))
    unknown = sorted(cited - known)
    ck("every plan citation exists in lookups.md", not unknown, f"unknown: {unknown}")
    for term in ("FR-26", "cache_only", "schema_fetch"):
        ck(f"plan mentions {term}", term in plan)
    print(f"{sum(results)}/{len(results)} passed")
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
