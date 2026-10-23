"""J16: compare Rust jsonschema verdicts with the host's Python jsonschema 4.26.0 (no retrieval). Reads j16-cases.json."""
import json, sys, importlib.metadata as m
from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry
cases = json.load(open(sys.argv[1]))
print(f"python {sys.version.split()[0]} jsonschema {m.version('jsonschema')} referencing {m.version('referencing')}")
agree = disagree = skipped = 0
for c in cases:
    if c["rust"] not in ("schema_valid", "schema_invalid"):
        skipped += 1; print(f"SKIP {c['id']}: rust={c['rust']}"); continue
    v = Draft202012Validator(c["schema"], format_checker=FormatChecker() if c["formats"] else None, registry=Registry())
    py = "schema_valid" if v.is_valid(c["instance"]) else "schema_invalid"
    same = py == c["rust"]; agree += same; disagree += not same
    print(f"{'AGREE   ' if same else 'DISAGREE'} {c['id']}: rust={c['rust']} python={py}")
print(f"SUMMARY agree={agree} disagree={disagree} skipped={skipped}")
sys.exit(0 if disagree == 0 else 1)
