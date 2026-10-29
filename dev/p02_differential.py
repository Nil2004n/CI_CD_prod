#!/usr/bin/env python3
"""P02-T14 differential B (planning/P02-T01-request.md §7, FR-16 / J16 style).

Compares the expected verdicts of the Rust store fixtures (persistence/tests/fixtures/store/manifest.json,
which the Rust tests already assert) with an independent validator: host Python `jsonschema` 4.26.0,
Draft 2020-12, format assertions on, every remote/file `$ref` retrieval refused. Comparator only: nothing is
installed, fetched, built or written outside --out. Run: python3 -I -B dev/p02_differential.py --out <dir>.

Expected schema-layer verdict per manifest class:
  valid, typed                          -> schema-valid (typed failures are stricter Rust checks)
  schema:<keyword>                      -> schema-invalid, and <keyword> is among the error keywords
  version:*, store_type_mismatch        -> schema-invalid (the Rust pre-check rejects them first)
Each fixture is checked twice: against `{"$ref": "#/$defs/<Root>File"}` (the per-root entry the Rust
validator compiles) and, for agreement with the published root, against the full root `oneOf`. A
store_type_mismatch fixture is a valid document of another root, so the full `oneOf` must accept it while
the per-root entry rejects it (run 1 of T14 expected `oneOf` to reject it: comparator defect, fixed).
"""
import argparse
import hashlib
import json
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SCHEMA = REPO / "schemas" / "workbench-store-v1.4.schema.json"
SCHEMA_RS = REPO / "workbench" / "crates" / "persistence" / "src" / "schema.rs"
FIXTURES = REPO / "workbench" / "crates" / "persistence" / "tests" / "fixtures" / "store"
EXPECTED_JSONSCHEMA = "4.26.0"
ROOT_DEF = {"config": "ConfigFile", "repositories": "RepositoryFile", "workflows": "WorkflowFile",
            "approvals": "ApprovalFile", "log": "LogFile", "suppressions": "SuppressionFile",
            "rag_index": "RagIndexFile", "conversations": "ConversationFile", "report": "ReportFile",
            "tool_manifest": "ToolManifestFile"}


REFUSED = []


def refuse(uri):
    REFUSED.append(uri)
    raise LookupError(f"remote/file $ref retrieval refused: {uri}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)

    from importlib.metadata import version
    from jsonschema import Draft202012Validator
    from referencing import Registry
    from referencing.exceptions import Unresolvable
    from referencing.jsonschema import DRAFT202012

    checks = []
    host = version("jsonschema")
    checks.append({"check": f"host jsonschema == {EXPECTED_JSONSCHEMA}", "value": host, "ok": host == EXPECTED_JSONSCHEMA})
    raw = SCHEMA.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    # T14 review L-9: the pin is read from the Rust source, so drift between the two is detected.
    pinned = re.search(r'SCHEMA_SHA256\s*:\s*&str\s*=\s*"([0-9a-f]{64})"', SCHEMA_RS.read_text())
    checks.append({"check": "schema sha256 == pinned (persistence::schema::SCHEMA_SHA256)", "value": sha,
                   "pinned": pinned.group(1) if pinned else None, "ok": bool(pinned) and sha == pinned.group(1)})
    schema = json.loads(raw)
    Draft202012Validator.check_schema(schema)
    resource = DRAFT202012.create_resource(schema)
    base = schema.get("$id", "urn:pwt:store-schema")
    registry = Registry(retrieve=refuse).with_resource(base, resource)
    fc = Draft202012Validator.FORMAT_CHECKER

    def validator(entry):
        return Draft202012Validator(entry, registry=registry, format_checker=fc)

    full = validator(schema)
    manifest = json.loads((FIXTURES / "manifest.json").read_text())
    rows, disagreements = [], []
    for f in manifest["fixtures"]:
        doc = json.loads((FIXTURES / f["file"]).read_text())
        exp = f["expect"]
        want_valid = exp in ("valid", "typed")
        per_root = validator({"$ref": f"{base}#/$defs/{ROOT_DEF[f['kind']]}"})
        errs = list(per_root.iter_errors(doc))
        def walk(err):  # T14 review L-9: every depth of `context`, not one level
            yield err
            for c in err.context:
                yield from walk(c)
        keywords = sorted({x.validator for e in errs for x in walk(e)})
        root_valid = full.is_valid(doc)
        want_root_valid = want_valid or exp == "store_type_mismatch"
        ok = (not errs) == want_valid and root_valid == want_root_valid
        if exp.startswith("schema:"):
            ok = ok and exp.split(":", 1)[1] in keywords
        row = {"file": f["file"], "kind": f["kind"], "expect": exp, "python_per_root_valid": not errs,
               "python_root_oneof_valid": root_valid, "python_keywords": keywords, "agree": ok}
        rows.append(row)
        if not ok:
            disagreements.append(row)
    checks.append({"check": "every fixture: host verdict agrees with the Rust-asserted expectation",
                   "fixtures": len(rows), "disagreements": disagreements, "ok": not disagreements and len(rows) > 0})

    # Negative controls: the comparator must be able to disagree.
    ctl = []
    valid_doc = json.loads((FIXTURES / "repositories.min.json").read_text())
    per_root = validator({"$ref": f"{base}#/$defs/RepositoryFile"})
    broken = dict(valid_doc, unexpected_key=1)
    ctl.append({"case": "extra key is schema-invalid", "ok": not per_root.is_valid(broken)})
    bad_fmt = json.loads(json.dumps(valid_doc))
    if isinstance(bad_fmt.get("updated_at"), str):
        bad_fmt["updated_at"] = "not-a-date"
        ctl.append({"case": "format assertion is on (bad date-time rejected)", "ok": not per_root.is_valid(bad_fmt)})
    # T14 security review L-5: the refusal must come from our retriever (counted), as `Unresolvable`, for
    # remote, file and relative references alike; any other exception fails the control.
    for ref in ("https://example.invalid/x.json", "file:///etc/hostname", "other.json"):
        before = len(REFUSED)
        try:
            validator({"$ref": ref}).is_valid({})
            ok = False
        except Unresolvable:
            ok = len(REFUSED) == before + 1 and REFUSED[-1].endswith(ref.rsplit("/", 1)[-1])
        ctl.append({"case": f"$ref refused by the retriever: {ref}", "ok": ok})
    checks.append({"check": "comparator negative controls", "cases": ctl, "ok": all(c["ok"] for c in ctl) and len(ctl) == 5})

    ok = all(c["ok"] for c in checks)
    (out / "differential.json").write_text(json.dumps({"checks": checks, "rows": rows, "ok": ok}, indent=2) + "\n")
    for c in checks:
        print(f"{'PASS' if c['ok'] else 'FAIL'} {c['check']}")
    print("OVERALL", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
