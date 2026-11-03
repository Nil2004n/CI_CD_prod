"""P04 development check runner (planning/P03-T01-request.md §5.8; derived from dev/p02_check.py, which stays
unchanged as the P02 record). Development only; not shipped.

Usage: python3 -I -B dev/p04_check.py <mode> --slot <lead|w1|w2|w3> --out <dir> [--package <crate>] [--final] [--repo <checkout>]
Modes: env, fmt, clippy, test, build, static, cli, all (= env fmt clippy test build cli static);
fmt-fix applies rustfmt in place (never part of `all`).
`cli` runs every scripted CLI scenario (`cargo test -p cli --tests`), headless.

Every Cargo command runs the locally selected 1.99.1 toolchain binaries directly, with `env -i` and the isolated
CARGO_HOME, inside `unshare -rn`, with --frozen (fmt has no network use). Exit codes are captured
from the producer process itself. Cargo.lock and every Cargo.toml are hashed before and after each step.
"""

import argparse
import os
import hashlib
import json
import re
import subprocess
import sys
import time
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent  # replaced by --repo
WS = REPO / "workbench"
PWT = Path("/tmp/p04-t04-devtools")
TC = Path("/home/debjoyy/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin")
TOOLCHAIN_VERSION = "1.99.1"
SLOTS = ("lead", "w1", "w2", "w3")
# Approved registry set (version, source, checksum), every entry equal to the P00 lock entry
# (evidence/P02/T02/record.md); keyed "name version" because syn appears in two versions.
# The set is unchanged in P03 (planning/P03-T01-request.md §4), so the P02 registry file is read as is.
REGISTRY_FILE = Path(__file__).resolve().parent / "p02_registry.json"
EXPECTED_REGISTRY = {k: tuple(v) for k, v in json.loads(REGISTRY_FILE.read_text())["registry"].items()}
MEMBERS = {"domain", "application", "repositories", "scanning", "retrieval", "persistence", "terminal-ui", "cli"}
# libc: P02-D03 O3 (owner 2026-10-27), constants only, already in the accepted graph.
ALLOWED_DIRECT_DEPS = {"serde", "serde_json", "jsonschema", "sha2", "libc", "domain", "persistence", "application", "repositories"}
TEST_RESULT = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out")


def target_dir(slot):
    """Per slot AND per repository checkout. Cargo's artifact names for workspace members do not depend on
    the workspace's absolute path, so two identically laid-out worktrees sharing one target directory can
    link each other's stale builds (found in P01 wave-1 verification, evidence/P01/T07/lead-verify)."""
    return PWT / "_cargo-target" / f"p04-{slot}-{hashlib.sha256(str(REPO).encode()).hexdigest()[:12]}"


def cargo_env(slot):
    return [
        f"PATH={TC}:/usr/bin:/bin",
        f"HOME={PWT}/_cargo-home",
        f"CARGO_HOME={PWT}/_cargo-home",
        f"CARGO_TARGET_DIR={target_dir(slot)}",
        f"RUSTC={TC}/rustc",
        f"RUSTDOC={TC}/rustdoc",
        "CARGO_TERM_COLOR=never",
        "CARGO_PROFILE_DEV_DEBUG=0",
        "CARGO_PROFILE_TEST_DEBUG=0",
        "LANG=C.UTF-8",
    ]


def protected_hashes():
    files = [WS / "Cargo.lock", WS / "Cargo.toml", WS / "clippy.toml", WS / "rustfmt.toml",
             *sorted((WS / "crates").glob("*/Cargo.toml"))]
    return {str(p.relative_to(REPO)): hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None for p in files}


def run_cargo(name, args, slot, out):
    log = out / f"{name}.log"
    before = protected_hashes()
    cmd = ["unshare", "-rn", "env", "-i", *cargo_env(slot), str(TC / "cargo"), *args]
    t0 = time.monotonic()
    with open(log, "wb") as fh:
        rc = subprocess.run(cmd, cwd=WS, stdout=fh, stderr=subprocess.STDOUT, check=False).returncode
    after = protected_hashes()
    res = {"step": name, "command": " ".join(["cargo", *args]), "exit": rc, "log": str(log.relative_to(out)),
           "seconds": round(time.monotonic() - t0, 2), "protected_files_unchanged": before == after}
    if before != after:
        res["changed"] = [k for k in before if before[k] != after.get(k)]
    res["ok"] = rc == 0 and before == after
    return res


def step_env(slot, out):
    checks = []
    for tool in ("rustc", "cargo", "rustdoc", "rustfmt", "cargo-clippy"):
        p = subprocess.run(["env", "-i", f"PATH={TC}:/usr/bin:/bin", str(TC / tool), "--version"],
                           capture_output=True, text=True, check=False)
        ver = p.stdout.strip()
        expect = TOOLCHAIN_VERSION if tool in ("rustc", "cargo", "rustdoc") else None
        ok = p.returncode == 0 and (expect is None or ver.split()[1:2] == [expect])
        checks.append({"check": f"{tool} --version", "value": ver, "ok": ok})
    # Compare namespace identities without attempting an external connection. A successful
    # unshare must create a different network namespace even if the parent sandbox blocks sockets.
    parent = os.readlink("/proc/self/ns/net")
    p = subprocess.run(["unshare", "-rn", "readlink", "/proc/self/ns/net"], capture_output=True, text=True, check=False)
    child = p.stdout.strip()
    checks.append({"check": "unshare creates distinct network namespace", "value": child,
                   "ok": p.returncode == 0 and bool(re.fullmatch(r"net:\[\d+\]", child)) and child != parent})
    checks.append({"check": f"slot {slot} target dir", "value": cargo_env(slot)[3], "ok": slot in SLOTS})
    (out / "env.json").write_text(json.dumps(checks, indent=2) + "\n")
    return {"step": "env", "checks": checks, "ok": all(c["ok"] for c in checks)}


def strip_comments(text, mask_strings=False):
    """Removes // and /* */ comments (doc comments included), keeping newlines so line numbers hold.
    String, raw-string and char literals are respected (a `//` inside a string is not a comment);
    with mask_strings their contents are replaced by spaces. Lifetimes are not mistaken for chars."""
    out, i, n = [], 0, len(text)
    char_lit = re.compile(r"b?'(\\.|\\u\{[0-9a-fA-F]+\}|[^\\'\n])'")
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append("".join(ch if ch == "\n" else " " for ch in text[i:j]))
            i = j
        elif (m := re.match(r'b?r(#*)"', text[i:i + 300])) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            close = '"' + m.group(1)
            j = text.find(close, i + m.end())
            j = n if j < 0 else j + len(close)
            body = text[i:j]
            out.append(body if not mask_strings else "".join(ch if ch == "\n" else " " for ch in body))
            i = j
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            j = min(j + 1, n)
            body = text[i:j]
            out.append(body if not mask_strings else '"' + "".join(ch if ch == "\n" else " " for ch in body[1:-1]) + '"')
            i = j
        elif c in "'b" and (m := char_lit.match(text, i)) and (c == "'" or text[i + 1] == "'"):
            out.append(m.group(0) if not mask_strings else " " * len(m.group(0)))
            i = m.end()
        else:
            out.append(c)
            i += 1
    return "".join(out)


def drop_test_modules(code):
    """Blanks `#[cfg(test)] mod <name> { ... }` blocks (keeping newlines) in comment-stripped code, so
    production-code checks do not count unit tests. Brace matching runs on string-masked code."""
    masked = strip_comments(code, mask_strings=True)
    out = list(code)
    for m in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", masked):
        depth, i = 1, m.end()
        while depth and i < len(masked):
            depth += {"{": 1, "}": -1}.get(masked[i], 0)
            i += 1
        for k in range(m.start(), i):
            if out[k] != "\n":
                out[k] = " "
    return "".join(out)


def line_of(code, pos):
    return code.count("\n", 0, pos) + 1


def code_lines(path, production=False):
    """Yields (lineno, line) of code with comments removed; with production, unit-test modules too."""
    code = strip_comments(path.read_text())
    yield from enumerate((drop_test_modules(code) if production else code).splitlines(), 1)


def split_top(s):
    """Splits at commas that are not nested inside (), [], {} or <>."""
    parts, depth, cur = [], 0, []
    for ch in s:
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return [p.strip() for p in parts if p.strip()]


def enum_variants(code, name):
    """Parses `pub enum <name> { ... }` from comment-stripped, string-masked code.
    Returns None if absent, else a list of (variant, kind, [(field, type)]) with kind unit/struct/tuple."""
    m = re.search(rf"\bpub\s+enum\s+{name}\b[^{{;]*\{{", code)
    if not m:
        return None
    depth, i = 1, m.end()
    while depth and i < len(code):
        depth += {"{": 1, "}": -1}.get(code[i], 0)
        i += 1
    body = re.sub(r"#\s*\[[^\]]*\]", " ", code[m.end():i - 1])
    variants = []
    for item in split_top(body):
        mv = re.match(r"(\w+)\s*(.*)$", item, re.S)
        if not mv:
            variants.append((item, "unparsed", []))
            continue
        vname, rest = mv.group(1), mv.group(2).strip()
        if rest == "" or rest.startswith("="):
            variants.append((vname, "unit", []))
        elif rest.startswith("{") and rest.endswith("}"):
            fields = []
            for f in split_top(rest[1:-1]):
                mf = re.match(r"(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:(?!:)\s*(.+)$", f, re.S)
                fields.append((mf.group(1), " ".join(mf.group(2).split())) if mf else (f, "<unparsed>"))
            variants.append((vname, "struct", fields))
        elif rest.startswith("("):
            variants.append((vname, "tuple", [("_", t) for t in split_top(rest[1:-1])]))
        else:
            variants.append((vname, "unparsed", []))
    return variants


ID_TYPES = {"ApprovalId", "ArtifactId", "CheckId", "ChunkId", "ConversationId", "EventId", "FileId", "FindingId",
            "MessageId", "RepoId", "ReportId", "RequestId", "RunId", "SourceId", "SuppressionId", "TaskId", "WorkflowId"}
DOMAIN_ENUMS = {"RunState", "CheckOutcome", "ApprovalState", "Capability", "Category", "Severity", "Assessment", "Coverage"}
EVENT_FIELD_TYPES = {"Option", "Vec", "u32", "bool", "UtcTimestamp", "RedactedText"} | ID_TYPES | DOMAIN_ENUMS
COMMAND_FIELD_TYPES = EVENT_FIELD_TYPES | {"RawInput"}
COMMAND_DATA_ENUMS = {"SelectionChange": COMMAND_FIELD_TYPES, "CredentialAction": {"CredentialSecret", "CredentialMode"}}


def check_contract(text, final):
    """Allowlist check of AppEvent/AppCommand field types (planning/P01-T01-request.md §5.5, §6.4).
    Every identifier in a field type must be allowlisted; contract-local types must be fieldless enums
    (or the listed command data enums, checked recursively); tuple variants are rejected."""
    code = strip_comments(text, mask_strings=True)
    checks = []

    def local_unit_enum(t):
        v = enum_variants(code, t)
        return v is not None and all(k == "unit" for _, k, _ in v)

    def bad_fields(enum, allowed, data_enums):
        bad = []
        for vname, kind, fields in enum_variants(code, enum) or []:
            if kind in ("tuple", "unparsed"):
                bad.append(f"{enum}::{vname}: {kind} variant not allowed")
            for fname, ftype in fields:
                for ident in re.findall(r"[A-Za-z_]\w*", ftype) or ["<none>"]:
                    if ident in allowed:
                        continue
                    if ident in data_enums and enum_variants(code, ident) is not None:
                        continue
                    if re.fullmatch(r"[A-Z]\w*", ident) and local_unit_enum(ident):
                        continue
                    bad.append(f"{enum}::{vname}.{fname}: `{ident}` in `{ftype}`")
        return bad

    for enum, allowed, data in (("AppEvent", EVENT_FIELD_TYPES, {}), ("AppCommand", COMMAND_FIELD_TYPES, COMMAND_DATA_ENUMS)):
        if enum_variants(code, enum) is None:
            checks.append({"check": f"{enum} field types allowlisted", "ok": not final,
                           "note": f"{enum} not present" + (" (required at --final)" if final else " yet")})
            continue
        bad = bad_fields(enum, allowed, data)
        for de, de_allowed in data.items():
            bad += bad_fields(de, (allowed | de_allowed), {})
        checks.append({"check": f"{enum} field types allowlisted", "hits": bad, "ok": not bad})
    return checks


def static_selftest():
    """Fixtures for every bypass reported by the P01 B1 security review (M3, L6); each must be caught."""
    head = "pub enum AppCommand { A }\n"
    must_fail = {
        "tuple variant": "pub enum AppEvent { A(String) }",
        "comma generic": "pub enum AppEvent { A { m: HashMap<u32, String> } }",
        "multi-line type": "pub enum AppEvent {\n A {\n m:\n Vec<\n String,\n >,\n },\n}",
        "alias": "type Msg = String;\npub enum AppEvent { A { m: Msg } }",
        "path type": "pub enum AppEvent { A { m: std::string::String } }",
        "OsString": "pub enum AppEvent { A { m: OsString } }",
        "PathBuf": "pub enum AppEvent { A { m: PathBuf } }",
        "Vec<u8>": "pub enum AppEvent { A { m: Vec<u8> } }",
        "Arc<str>": "pub enum AppEvent { A { m: Arc<str> } }",
        "Box<dyn Error>": "pub enum AppEvent { A { m: Box<dyn Error> } }",
        "brace in doc comment": "pub enum AppEvent {\n /// see }\n A { a: u32 },\n /// {\n B { b: String },\n}",
        "local enum with data": "pub enum Leak { X(String) }\npub enum AppEvent { A { m: Leak } }",
        "raw string in command": "pub enum AppCommand { A { url: String } }\npub enum AppEvent { B }",
        "SanitisedText in event": "pub enum AppEvent { A { m: SanitisedText } }",
        "tuple variant in SelectionChange": "pub enum SelectionChange { X(String) }\npub enum AppCommand { A { c: SelectionChange } }\npub enum AppEvent { B }",
        "CredentialSecret outside CredentialAction": "pub enum AppCommand { A { s: CredentialSecret } }\npub enum AppEvent { B }",
    }
    results = []
    for name, src in must_fail.items():
        text = src if "AppCommand" in src else head + src
        ok = not all(c["ok"] for c in check_contract(text, final=True))
        results.append({"case": f"rejects {name}", "ok": ok})
    good = ("pub enum Flag { On, Off }\npub enum AppCommand { A { url: RawInput, ids: Vec<RepoId> } }\n"
            "pub enum AppEvent {\n /// doc with { brace\n A { r: RepoId, t: Option<RedactedText>, f: Flag },\n B,\n}")
    results.append({"case": "accepts allowlisted contract", "ok": all(c["ok"] for c in check_contract(good, final=True))})
    results.append({"case": "final requires both enums", "ok": not all(c["ok"] for c in check_contract("pub enum AppEvent { B }", final=True))})
    hidden = 'let s = "a//b"; unsafe { x() }\nlet u = "c//d"; println!("x");'
    stripped = strip_comments(hidden)
    results.append({"case": "unsafe after // in string is visible", "ok": bool(re.search(r"\bunsafe\b", stripped))})
    results.append({"case": "println after // in string is visible", "ok": bool(re.search(PRINT_RE, stripped))})
    results.append({"case": "comment text is removed", "ok": "unsafe" not in strip_comments("// unsafe { }\n/* println!() */")})
    rd = re.compile(r"\bimpl\b[^{;]*\bRedactor\s+for\b")
    in_test = "fn a() {}\n#[cfg(test)]\nmod tests {\n    struct I;\n    impl Redactor for I { fn x() { let s = \"}\"; } }\n}\n"
    results.append({"case": "Redactor impl inside #[cfg(test)] mod is ignored", "ok": not rd.search(drop_test_modules(strip_comments(in_test)))})
    results.append({"case": "Redactor impl outside test module is caught",
                    "ok": bool(rd.search(drop_test_modules(strip_comments(in_test + "impl Redactor for Leak {}\n"))))})
    wrapped = "impl\n    Redactor\n    for Leak {}"
    results.append({"case": "wrapped Redactor impl header is caught", "ok": bool(re.search(REDACTOR_IMPL_RE, wrapped, re.S))})
    results.append({"case": "lifetime is not a char literal", "ok": "unsafe" in strip_comments("fn f<'a>(x: &'a str) { unsafe {} }")})
    return results


# P04-T02 edges, except the owner-approved removal of the release-reachable fixture self-edge.
EXPECTED_LOCK_SHA256 = "04dde42d2cee66672982e11f0df3125ee436f0f3da7713f57b0bbb58ce33f87f"
# The only `#[allow(...)]` attributes permitted in production code (file, lint) — planning/P01-T01-request.md §6.1.
SRC_ALLOWS = {("crates/domain/src/contract.rs", "clippy::result_large_err"),
              # P02-D04: `store_read::redacted_vec` has no store field yet (reserved for list-valued class R fields).
              ("crates/domain/src/text.rs", "dead_code")}
# The only production call sites of `redact_static` (file); argument must be a literal or ALL_CAPS const.
REDACT_STATIC_CALLERS = {"crates/domain/src/contract.rs"}
REDACTOR_IMPL_RE = r"\bimpl\b[^{;]*?\bRedactor\b[^{;]*?\bfor\b"

PRINT_RE = r"\b(e?print(ln)?|dbg)\s*!|\bio::std(out|err)\s*\(|\bstd(out|err)\s*\(\s*\)"
PRINT_MACRO_RE = r"\b(e?print(ln)?|dbg)\s*!"
# The binary entry point is the only production file that may take the process stdout/stderr handles.
CLI_MAIN = "crates/cli/src/main.rs"


def step_static(final, out):
    checks = []
    rs = sorted(WS.rglob("*.rs"))
    src = [p for p in rs if (WS / "crates") in p.parents and p.relative_to(WS / "crates").parts[1:2] == ("src",)]
    hits = [f"{p.relative_to(REPO)}:{i}" for p in rs
            for i, l in enumerate(strip_comments(p.read_text(), mask_strings=True).splitlines(), 1)
            if re.search(r"\bunsafe\b", l)]
    checks.append({"check": "no `unsafe` in code (comments and strings masked)", "hits": hits, "ok": not hits})
    hits = [f"{p.relative_to(REPO)}:{i}" for p in src for i, l in code_lines(p, production=True)
            if re.search(PRINT_MACRO_RE if p == WS / CLI_MAIN else PRINT_RE, l)]
    checks.append({"check": "no print/dbg/stdout/stderr in src", "hits": hits, "ok": not hits})
    def rel(p):
        return str(p.relative_to(WS))

    files = {rel(p): p.read_text() for p in src}  # raw text of every `src` file, keyed by workspace-relative path
    checks.append(check_todo({rel(p): p.read_text() for p in rs}, final))

    prod = {p: drop_test_modules(strip_comments(p.read_text())) for p in src}
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items()
            for m in re.finditer(REDACTOR_IMPL_RE + r"|\bRedactor\s+as\b", c, re.S)]
    checks.append({"check": "no Redactor impl or alias in src outside #[cfg(test)] modules (until P06)", "hits": hits, "ok": not hits})
    hits = []
    for p, c in prod.items():
        masked = drop_test_modules(strip_comments(p.read_text(), mask_strings=False))
        for m in re.finditer(r"\bredact_static\s*\(\s*([^)]*)\)", masked):
            arg = m.group(1).strip()
            if rel(p) == "crates/domain/src/text.rs" and arg.startswith("text:"):
                continue  # the definition itself
            if rel(p) not in REDACT_STATIC_CALLERS or not re.fullmatch(r'"[^"{}\\]*"|[A-Z][A-Z0-9_]*', arg):
                hits.append(f"{rel(p)}:{line_of(c, m.start())}: {arg[:60]}")
    checks.append({"check": "redact_static called only from allowlisted files with a literal or ALL_CAPS const", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() for m in re.finditer(r"\bBox\s*::\s*leak\b|\.leak\s*\(", c)]
    checks.append({"check": "no Box::leak / .leak() in src", "hits": hits, "ok": not hits})
    checks.append(check_allows(files, final))
    # Closed construction of RedactedText (reviewer M1): trait impls for it anywhere in src must be exactly
    # the approved set, and its derive list is fixed.
    impls = set()
    for p, c in prod.items():
        for m in re.finditer(r"\bimpl\b(?:\s*<[^>]*>)?\s*([\w:]+(?:\s*<[^>]*>)?)\s+for\s+(?:[\w:]*::)?RedactedText\b", c):
            impls.add((rel(p), re.sub(r"\s+", "", m.group(1))))
    approved = {("crates/domain/src/text.rs", "fmt::Display"), ("crates/domain/src/text.rs", "serde::Serialize")}
    derive = re.search(r"#\[derive\(([^)]*)\)\]\s*pub\s+struct\s+RedactedText\b", strip_comments((WS / "crates/domain/src/text.rs").read_text()))
    derives = sorted(d.strip() for d in derive.group(1).split(",")) if derive else None
    checks.append({"check": "RedactedText trait impls == {Display, Serialize}; derives == {Clone, Debug, Eq, Hash, PartialEq}",
                   "impls": sorted(map(list, impls)), "derives": derives,
                   "ok": impls == approved and derives == ["Clone", "Debug", "Eq", "Hash", "PartialEq"]})
    build_code = [str(x.relative_to(REPO)) for x in sorted(WS.rglob("build.rs"))]
    proc_macro = [rel(m) for m in sorted((WS / "crates").glob("*/Cargo.toml"))
                  if tomllib.loads(m.read_text()).get("lib", {}).get("proc-macro")]
    cfgs = [str(d) for base in [REPO, *REPO.parents] for d in [base / ".cargo" / "config", base / ".cargo" / "config.toml",
            base / "rust-toolchain", base / "rust-toolchain.toml"] if d.exists()]
    cfgs += [str(x) for x in WS.rglob(".cargo") if x.is_dir()]
    checks.append({"check": "no workspace build.rs, proc-macro crate, .cargo/config or rust-toolchain file (repo and ancestors)",
                   "build_rs": build_code, "proc_macro": proc_macro, "config": cfgs, "ok": not (build_code or proc_macro or cfgs)})
    contract = WS / "crates/domain/src/contract.rs"
    checks += check_contract(contract.read_text() if contract.exists() else "", final)
    checks += check_p02(prod, rel, line_of)
    checks += check_p03(prod_map(files))
    checks += check_p04(prod_map(files))
    manifests = [WS / "Cargo.toml", *sorted((WS / "crates").glob("*/Cargo.toml"))]
    feature_hits = [str(p.relative_to(WS)) for p in manifests if "fixture-transport" in p.read_text()]
    checks.append({"check": "no fixture-transport feature in workspace manifests (blocked until isolation design)",
                   "hits": feature_hits, "ok": not feature_hits})
    st = static_selftest() + p02_selftest() + p03_selftest() + p04_selftest()
    checks.append({"check": "static checker self-test", "cases": st, "ok": all(c["ok"] for c in st)})
    lock = tomllib.loads((WS / "Cargo.lock").read_text())["package"]
    reg = {(f'{p["name"]} {p["version"]}', (p["version"], p["source"], p.get("checksum"))) for p in lock if p.get("source")}
    mem = {p["name"] for p in lock if not p.get("source")}
    checks.append({"check": "Cargo.lock registry set == approved (version, source, checksum)",
                   "extra": sorted(map(list, reg - set(EXPECTED_REGISTRY.items()))),
                   "missing": sorted(map(list, set(EXPECTED_REGISTRY.items()) - reg)),
                   "ok": reg == set(EXPECTED_REGISTRY.items())})
    checks.append({"check": "Cargo.lock members == 8 workspace crates", "ok": mem == MEMBERS})
    lock_sha = hashlib.sha256((WS / "Cargo.lock").read_bytes()).hexdigest()
    checks.append({"check": "Cargo.lock sha256 == P04 approved dependency edges", "value": lock_sha, "ok": lock_sha == EXPECTED_LOCK_SHA256})
    direct = set()
    for man in [WS / "Cargo.toml", *sorted((WS / "crates").glob("*/Cargo.toml"))]:
        t = tomllib.loads(man.read_text())
        for tbl in (t.get("workspace", {}).get("dependencies", {}), t.get("dependencies", {}),
                    t.get("dev-dependencies", {}), t.get("build-dependencies", {})):
            direct |= set(tbl)
        for k in t.get("target", {}).values():
            for tbl in ("dependencies", "dev-dependencies", "build-dependencies"):
                direct |= set(k.get(tbl, {}))
    checks.append({"check": "manifest dependencies within the P02 allowlist", "found": sorted(direct),
                   "ok": direct <= ALLOWED_DIRECT_DEPS})
    (out / "static.json").write_text(json.dumps(checks, indent=2) + "\n")
    return {"step": "static", "checks": checks, "ok": all(c["ok"] for c in checks)}


# --- P02 static checks (planning/P02-T01-request.md §8) -------------------------------------------------
STORE_DIR = "crates/domain/src/store/"
STORE_FIELD_EXEMPT = {"crates/domain/src/store/values.rs", "crates/domain/src/store/mod.rs"}
BARE_TEXT_RE = r"\b(String|str|Cow|Value)\b"
STORE_READ_OK = ("crates/domain/src/text.rs", STORE_DIR)
SERDE_FROM_RE = r"\bserde_json\s*::\s*from_(str|slice|value|reader)\b|\bfrom_(str|slice|value|reader)\s*::\s*<"
# P03: + the dev permission-check DTO decoder (planning/P03-T01-request.md §5.8).
SERDE_FROM_OK = {"crates/persistence/src/read.rs", "crates/persistence/src/schema.rs",
                 "crates/application/src/dev_permission.rs"}
# T14 review L-6: also brace imports (`use std::{fs, ..}`) and any bare `fs::<item>` path (not `x::fs::`).
FS_RE = r"\bstd\s*::\s*\{[^}]*\bfs\b|(?<![\w:])fs\s*::\s*\w+|\bstd\s*::\s*fs\b|\bfs\s*::\s*(File|OpenOptions|read|write|rename|remove_file|create_dir|hard_link|metadata|symlink_metadata|read_dir|copy)\b|\bFile\s*::|\bOpenOptions\b"
FS_OK = {"crates/persistence/src/fs.rs", "crates/application/src/emit.rs"}
# P03: every `src` file of every crate is in scope for environment reads; only cli/src/main.rs may read them
# (ENV_OK, below). Brace imports (`use std::env::{var, vars}`) and the glob import are caught as well;
# `env::args` / `env::args_os` stay allowed.
_ENV_ITEMS = r"(?:var|var_os|vars|vars_os|home_dir|temp_dir|current_dir)"
ENV_RE = (rf"\benv\s*::\s*{_ENV_ITEMS}\b|\benv\s*::\s*\{{[^}}]*\b{_ENV_ITEMS}\b|\benv\s*::\s*\*"
          r"|\bhome_dir\s*\(")
# Scope of the P02 store-Event construction rule (unchanged).
ENV_SCOPE = ("crates/cli/src/", "crates/application/src/", "crates/persistence/src/")
# P02-T14 additions (wave 3).
CLI_SRC = "crates/cli/src/"
LAYER_RE = r"\bpersistence\s*::|\buse\s+persistence\b|\bextern\s+crate\s+persistence\b"
EXIT_RE = (r"\bprocess\s*::\s*(exit|abort)\b|\bstd\s*::\s*process\s*::\s*exit\b"
           r"|\buse\s+std\s*::\s*process\s*::\s*\{[^}]*\b(exit|abort)\b")
URANDOM_RE = r"/dev/u?random"
URANDOM_OK = FS_OK
STORE_EVENT_RE = r"(?<!struct )(?<!enum )\bEvent\s*\{"
STORE_EVENT_OK = {"crates/application/src/emit.rs"}
# Fault-injection seams of application::dev_store (made pub in T14): no production caller outside dev_store.rs.
SEAM_RE = r"\b(init|validate|show|recover)_with\s*\("
SEAM_OK = {"crates/application/src/dev_store.rs"}


def struct_fields(code):
    """Yields (struct, field, type) for `struct X { ... }` bodies of comment-stripped, string-masked code."""
    for m in re.finditer(r"\bstruct\s+(\w+)\s*(?:<[^>{]*>)?\s*\{", code):
        depth, i = 1, m.end()
        while depth and i < len(code):
            depth += {"{": 1, "}": -1}.get(code[i], 0)
            i += 1
        body = re.sub(r"#\s*\[[^\]]*\]", " ", code[m.end():i - 1])
        for f in split_top(body):
            mf = re.match(r"(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:(?!:)\s*(.+)$", f, re.S)
            if mf:
                yield m.group(1), mf.group(1), " ".join(mf.group(2).split())


def store_field_hits(code):
    return [f"{s}.{f}: {ty}" for s, f, ty in struct_fields(code) if re.search(BARE_TEXT_RE, ty)]


def check_p02(prod, rel, line_of):
    checks = []
    masked = {p: strip_comments(p.read_text(), mask_strings=True) for p in prod}
    hits = []
    for p in prod:
        r = rel(p)
        if r.startswith(STORE_DIR) and r not in STORE_FIELD_EXEMPT:
            hits += [f"{r}: {h}" for h in store_field_hits(drop_test_modules(masked[p]))]
    checks.append({"check": "store-model fields: no bare String/str/Cow/Value (FR-31 (3))", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if not rel(p).startswith(STORE_READ_OK)
            for m in re.finditer(r"\bstore_read\b", c)]
    checks.append({"check": "text::store_read used only in domain/src/text.rs and domain/src/store/", "hits": hits, "ok": not hits})
    # serde_json::from_* (SERDE_FROM_OK) and the environment rule (ENV_RE) are file-map rules in check_p03.
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p) not in FS_OK
            for m in re.finditer(FS_RE, c)]
    checks.append({"check": "std::fs / File / OpenOptions only in persistence/src/fs.rs and application/src/emit.rs", "hits": hits, "ok": not hits})
    # --- P02-T14 (wave 3) ---
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p).startswith(CLI_SRC)
            for m in re.finditer(LAYER_RE, c)]
    checks.append({"check": "cli src does not reference persistence (CLI -> application only)", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() for m in re.finditer(EXIT_RE, c)]
    checks.append({"check": "no process::exit/abort in src (exit codes via ExitCode)", "hits": hits, "ok": not hits})
    raw = {p: drop_test_modules(strip_comments(p.read_text(), mask_strings=False)) for p in prod}
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in raw.items() if rel(p) not in URANDOM_OK
            for m in re.finditer(URANDOM_RE, c)]
    checks.append({"check": "/dev/urandom only in persistence/src/fs.rs and application/src/emit.rs", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p).startswith(ENV_SCOPE)
            and rel(p) not in STORE_EVENT_OK for m in re.finditer(STORE_EVENT_RE, c)]
    checks.append({"check": "store log Event records constructed only in application/src/emit.rs (to_store_event)", "hits": hits, "ok": not hits})
    hits = []
    for p in prod:
        r = rel(p)
        if r.startswith(STORE_DIR) and r not in STORE_FIELD_EXEMPT:
            hits += [f"{r}: {h}" for h in redacted_field_hits(drop_test_modules(strip_comments(p.read_text(), mask_strings=False)))]
    checks.append({"check": "store RedactedText fields deserialise through text::store_read (FR-31 (2))", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p) not in SEAM_OK
            for m in re.finditer(SEAM_RE, c)]
    checks.append({"check": "dev_store *_with fault seams called only from dev_store.rs and tests", "hits": hits, "ok": not hits})
    return checks


def redacted_field_hits(code):
    """Store struct fields whose type mentions RedactedText but whose own attributes lack a
    `deserialize_with = "...store_read::redacted..."` (comment-stripped code, strings kept)."""
    hits = []
    for m in re.finditer(r"\bstruct\s+(\w+)\s*(?:<[^>{]*>)?\s*\{", code):
        depth, i = 1, m.end()
        while depth and i < len(code):
            depth += {"{": 1, "}": -1}.get(code[i], 0)
            i += 1
        body = code[m.end():i - 1]
        for f in split_top(body):
            attrs = " ".join(re.findall(r"#\s*\[[^\]]*\]", f))
            decl = re.sub(r"#\s*\[[^\]]*\]", " ", f)
            mf = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:(?!:)\s*(.+)$", decl, re.S)
            if mf and re.search(r"\bRedactedText\b", mf.group(2)) and not re.search(
                    r'deserialize_with\s*=\s*"[\w:]*store_read::redacted(_opt|_vec|_map)?"', attrs):
                hits.append(f"{m.group(1)}.{mf.group(1)}")
    return hits


def p02_selftest():
    """Negative controls for the P02 static checks; each must be caught."""
    res = []
    bad_fields = {
        "String field": "pub struct A { pub name: String }",
        "Option<String>": "pub struct A { pub name: Option<String> }",
        "Vec<String>": "pub struct A { pub names: Vec<String> }",
        "Cow": "pub struct A<'a> { pub n: Cow<'a, str> }",
        "Box<str>": "pub struct A { pub n: Box<str> }",
        "serde_json::Value": "pub struct A { pub v: serde_json::Value }",
        "multi-line": "pub struct A {\n    #[serde(default)]\n    pub n:\n        String,\n}",
    }
    for name, s in bad_fields.items():
        res.append({"case": f"store field rejects {name}", "ok": bool(store_field_hits(strip_comments(s, mask_strings=True)))})
    good = "pub struct A { pub id: RepoId, pub t: Option<RedactedText>, pub s: Vec<Slug>, pub n: u32 }"
    res.append({"case": "store field accepts typed fields", "ok": not store_field_hits(strip_comments(good, mask_strings=True))})
    res.append({"case": "store field ignores String in a doc comment",
                "ok": not store_field_hits(strip_comments("pub struct A {\n /// a String\n pub n: Slug,\n}", mask_strings=True))})
    for s in ("let x = serde_json::from_str::<ConfigFile>(t);", "let x: R = from_value::<R>(v);", "serde_json :: from_slice(b)"):
        res.append({"case": f"serde from_* caught: {s[:30]}", "ok": bool(re.search(SERDE_FROM_RE, s))})
    for s in ("std::fs::read(p)", "use std::fs;", "File::open(p)", "OpenOptions::new()", "fs::rename(a, b)",
              "use std::{fs, io};", "fs::read_to_string(p)", "fs::create_dir_all(p)", "fs::set_permissions(p, m)"):
        res.append({"case": f"fs use caught: {s}", "ok": bool(re.search(FS_RE, s))})
    res.append({"case": "fs regex ignores FsOps trait use", "ok": not re.search(FS_RE, "fs.rename(&a, &b)?; let f: &dyn FsOps = x;")})
    res.append({"case": "fs regex ignores the persistence::fs module path",
                "ok": not re.search(FS_RE, "use persistence::fs::{FsOps, RealFs}; crate::fs::FileInfo")})
    for s in ("std::env::var(\"HOME\")", "env::var_os(k)", "std::env::temp_dir()", "env::home_dir()", "std::env::current_dir()"):
        res.append({"case": f"env lookup caught: {s}", "ok": bool(re.search(ENV_RE, s))})
    res.append({"case": "env regex allows args_os", "ok": not re.search(ENV_RE, "std::env::args_os()")})
    res.append({"case": "print macro regex catches println in main", "ok": bool(re.search(PRINT_MACRO_RE, 'println!("x")'))})
    res.append({"case": "print macro regex allows stdout handle in main", "ok": not re.search(PRINT_MACRO_RE, "std::io::stdout().lock()")})
    # --- P02-T14 negative and positive controls ---
    for s in ("use persistence::writer::StoreWriter;", "let w = persistence :: writer::open(d);", "extern crate persistence;"):
        res.append({"case": f"layer rule catches: {s[:32]}", "ok": bool(re.search(LAYER_RE, s))})
    res.append({"case": "layer rule allows application::dev_store", "ok": not re.search(LAYER_RE, "use application::dev_store;")})
    for s in ("std::process::exit(2)", "process::abort()", "std :: process :: exit(1);"):
        res.append({"case": f"exit rule catches: {s}", "ok": bool(re.search(EXIT_RE, s))})
    res.append({"case": "exit rule allows ExitCode", "ok": not re.search(EXIT_RE, "ExitCode::from(2)")})
    res.append({"case": "exit rule catches a brace import", "ok": bool(re.search(EXIT_RE, "use std::process::{abort, Command};"))})
    res.append({"case": "urandom rule catches a literal", "ok": bool(re.search(URANDOM_RE, 'File::open("/dev/urandom")'))})
    res.append({"case": "store Event literal caught", "ok": bool(re.search(STORE_EVENT_RE, "Some(Event { event_id: x })"))})
    res.append({"case": "store Event definition ignored", "ok": not re.search(STORE_EVENT_RE, "pub struct Event { pub a: u8 }")})
    res.append({"case": "EventEnvelope literal not a store Event", "ok": not re.search(STORE_EVENT_RE, "EventEnvelope { event_id }")})
    bad_r = {
        "missing helper": "pub struct A { pub t: RedactedText }",
        "wrong helper": 'pub struct A {\n #[serde(deserialize_with = "crate::x::other")]\n pub t: RedactedText,\n}',
        "Option without helper": "pub struct A { pub t: Option<RedactedText>, pub n: u32 }",
        "helper on the wrong field": 'pub struct A {\n #[serde(deserialize_with = "crate::text::store_read::redacted")]\n pub n: u32,\n pub t: RedactedText,\n}',
    }
    for name, s in bad_r.items():
        res.append({"case": f"store R field rule catches {name}", "ok": bool(redacted_field_hits(s))})
    good_r = ('pub struct A {\n #[serde(deserialize_with = "crate::text::store_read::redacted")]\n pub t: RedactedText,\n'
              ' #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]\n pub o: Option<RedactedText>,\n pub n: u32,\n}')
    res.append({"case": "store R field rule accepts helpered fields", "ok": not redacted_field_hits(good_r)})
    for s in ("dev_store::validate_with(fs, v, d, None)", "recover_with (a, b, c, d)"):
        res.append({"case": f"seam rule catches: {s[:28]}", "ok": bool(re.search(SEAM_RE, s))})
    res.append({"case": "seam rule allows the production entry points", "ok": not re.search(SEAM_RE, "dev_store::validate(d, None)")})
    return res


# --- P03 static checks (planning/P03-T01-request.md §5.8) -----------------------------------------------
# Every rule below is a function of a file map (workspace-relative path -> text), so the self-test drives the
# same code as the real scan. `prod_map` gives the comment-stripped, test-module-free production code.
SRC_PATH_RE = re.compile(r"^crates/[^/]+/src/")
ENV_OK = {CLI_MAIN}
PROCESS_RE = (r"\bprocess\s*::\s*(?:Command\b|\*)|\bprocess\s*::\s*\{[^}]*(?:\bCommand\b|\*)"
              r"|\bCommand\s*::\s*new\b")
SECRET_FILES = {"crates/domain/src/contract.rs", "crates/application/src/credential.rs"}
EXPOSE_OK = SECRET_FILES
SECRET_NEW_OK = SECRET_FILES
SECRET_NEW_RE = r"\bCredentialSecret\s*::\s*new\b|\bCredentialSecret\s+as\b"
REDACT_VALIDATED_OK = {"crates/domain/src/store/values.rs"}
REDACT_VALIDATED_DEF = "crates/domain/src/text.rs"
AUTHORISATION_OK = {"crates/application/src/approvals.rs"}
GRANT_DIR = "crates/application/src/permission/"
# T11 (P03-D01 H1): only approvals.rs acts on `Evaluation::Permitted` and calls `permission::evaluate`; the
# permission module itself defines and produces them.
PERMITTED_OK = {"crates/application/src/approvals.rs"}
PERMITTED_RE = r"\bEvaluation\s*::\s*(?:Permitted\b|\*|\{[^}]*(?:\bPermitted\b|\*))"
# P03-D02 C1: `RequestOutcome::Permitted` is an unconsumed "a grant exists" answer, never a go-ahead; only
# approvals.rs (which defines and returns it) may name it in src.
REQUEST_PERMITTED_RE = r"\bRequestOutcome\s*::\s*(?:Permitted\b|\*|\{[^}]*(?:\bPermitted\b|\*))"
EVALUATE_RE = r"\bpermission\s*::\s*(?:evaluate\b|\*|\{[^}]*(?:\bevaluate\b|\*))"
TODO_RE = r"\btodo\s*!|\bunimplemented\s*!|clippy::todo"
ALLOW_RE = r"#!?\s*\[\s*(?:cfg_attr\s*\([^\[\]]*?,\s*)?(?:allow|expect)\s*\(([^)]*)"
# What may precede `Name {` without it being a struct-literal: a declaration, an impl header or a return type.
DECL_BEFORE_RE = re.compile(r"(?:\b(?:struct|enum|union|trait|impl|for)\s+|->\s*)$")


def prod_map(files):
    """Production code (comments and #[cfg(test)] modules removed, strings kept) of the `src` files of a file map."""
    return {r: drop_test_modules(strip_comments(t)) for r, t in files.items() if SRC_PATH_RE.match(r)}


def scan(prod, regex, allowed=(), skip=None):
    """`path:line` of every match of regex outside the allowed paths; skip(code, match) can exempt a match."""
    return [f"{r}:{line_of(c, m.start())}" for r, c in sorted(prod.items()) if r not in allowed
            for m in re.finditer(regex, c, re.S) if not (skip and skip(c, m))]


def env_hits(prod):
    return scan(prod, ENV_RE, ENV_OK)


def process_hits(prod):
    return scan(prod, PROCESS_RE, {"crates/repositories/src/git/runner.rs"})


def expose_hits(prod):
    return scan(prod, r"\bexpose\b(?!_)", EXPOSE_OK)


def secret_new_hits(prod):
    return scan(prod, SECRET_NEW_RE, SECRET_NEW_OK)


def serde_from_hits(prod):
    return scan(prod, SERDE_FROM_RE, SERDE_FROM_OK)


def redact_validated_hits(prod):
    """Any mention (call, path, import, function pointer) outside values.rs, except the `fn redact_validated`
    definition in text.rs."""
    return [f"{r}:{line_of(c, m.start())}" for r, c in sorted(prod.items()) if r not in REDACT_VALIDATED_OK
            for m in re.finditer(r"\bredact_validated\b", c)
            if not (r == REDACT_VALIDATED_DEF and re.search(r"\bfn\s+$", c[max(0, m.start() - 200):m.start()]))]


def literal_hits(prod, name, allowed):
    """Struct-literal constructions `Name {` outside the allowed paths. Declarations (`struct Name {`),
    impl headers (`impl Name {`, `impl T for Name {`) and return types (`-> Name {`) are not constructions."""
    return [f"{r}:{line_of(c, m.start())}" for r, c in sorted(prod.items()) if not allowed(r)
            for m in re.finditer(rf"\b{name}\s*\{{", c)
            if not DECL_BEFORE_RE.search(c[max(0, m.start() - 200):m.start()])]


def authorisation_hits(prod):
    return literal_hits(prod, "Authorisation", lambda r: r in AUTHORISATION_OK)


def grant_hits(prod):
    return literal_hits(prod, "Grant", lambda r: r.startswith(GRANT_DIR))


def permitted_hits(prod):
    return [h for h in scan(prod, PERMITTED_RE, PERMITTED_OK) if not h.startswith(GRANT_DIR)]


def evaluate_hits(prod):
    return [h for h in scan(prod, EVALUATE_RE, PERMITTED_OK) if not h.startswith(GRANT_DIR)]


def request_permitted_hits(prod):
    return scan(prod, REQUEST_PERMITTED_RE, PERMITTED_OK)


def check_p03(prod):
    checks = []
    for label, fn in (
            ("environment reads (env::var*/vars*/home_dir/temp_dir/current_dir) only in cli/src/main.rs", env_hits),
            ("std::process::Command / Command::new only in repositories/src/git/runner.rs", process_hits),
            ("CredentialSecret::expose not referenced in src outside domain/src/contract.rs and application/src/credential.rs", expose_hits),
            ("CredentialSecret::new (and `CredentialSecret as`) only in domain/src/contract.rs and application/src/credential.rs", secret_new_hits),
            ("serde_json::from_* only in persistence/src/{read,schema}.rs and application/src/dev_permission.rs", serde_from_hits),
            ("redact_validated referenced only in domain/src/store/values.rs (definition in text.rs)", redact_validated_hits),
            ("Authorisation { .. } constructed only in application/src/approvals.rs", authorisation_hits),
            ("Grant { .. } constructed only under application/src/permission/", grant_hits),
            ("Evaluation::Permitted acted on only in application/src/approvals.rs (and permission/)", permitted_hits),
            ("permission::evaluate referenced only in application/src/approvals.rs (and permission/)", evaluate_hits),
            ("RequestOutcome::Permitted named only in application/src/approvals.rs (unconsumed; never a go-ahead)", request_permitted_hits)):
        hits = fn(prod)
        checks.append({"check": label, "hits": hits, "ok": not hits})
    return checks


# P04 lexical policy checks are defense in depth, not proof of Rust call graphs or
# Cargo feature isolation. Release-path compilation and manual review remain required.
GIT_RUNNER = "crates/repositories/src/git/runner.rs"
GIT_OPS = "crates/repositories/src/git/ops.rs"
SHELL_RE = r'(?i)(?:["\'](?:[^"\']*/)?(?:ba)?sh["\']|\b(?:ba)?sh\b)'
RUNNER_RE = r"\b(?:GitRunner|RealGitRunner|ProcessGitRunner|GitInvocation|git\s*::\s*runner)\b"
FIXTURE_RE = r"\b(?:fixture_transport|FixtureTransport|file_transport|FileTransport)\b"


def check_p04(prod):
    # Conservative lexical bans; neither passing these nor unit tests proves the
    # transport is unreachable or that arbitrary shell invocation is impossible.
    shell = scan(prod, SHELL_RE)
    runner = scan(prod, RUNNER_RE, {GIT_RUNNER, GIT_OPS})
    fixture = scan(prod, FIXTURE_RE)
    return [{"check": label, "hits": hits, "ok": not hits} for label, hits in (
        ("no shell executable references in production source (lexical)", shell),
        ("Git runner interfaces referenced only by runner and hardened operations (lexical)", runner),
        ("no fixture transport identifiers in production source (lexical; release proof pending)", fixture))]


def p04_selftest():
    def checks(files):
        return check_p04(prod_map(files))
    a = "crates/application/src/x.rs"
    return [
        {"case": "P04 unsafe literal masked while unsafe block remains", "ok": not re.search(r"\bunsafe\b", strip_comments('let s = "unsafe";', mask_strings=True)) and bool(re.search(r"\bunsafe\b", strip_comments('let s = "unsafe"; unsafe {}', mask_strings=True)))},
        {"case": "P04 shell literal rejected", "ok": not checks({GIT_RUNNER: 'Command::new("/bin/sh").arg("-c")'})[0]["ok"]},
        {"case": "P04 direct runner rejected", "ok": not checks({a: "let _: RealGitRunner = x;"})[1]["ok"]},
        {"case": "P04 process runner rejected", "ok": not checks({a: "let _: ProcessGitRunner = x;"})[1]["ok"]},
        {"case": "P04 direct runner module rejected", "ok": not checks({a: "use repositories::git::runner::GitError;"})[1]["ok"]},
        {"case": "P04 runner allowed in operations", "ok": checks({GIT_OPS: "let _: GitRunner = x;"})[1]["ok"]},
        {"case": "P04 fixture identifier rejected", "ok": not checks({GIT_RUNNER: "let fixture_transport = true;"})[2]["ok"]},
        {"case": "P04 unit test module excluded", "ok": checks({a: '#[cfg(test)] mod tests { fn t() { let fixture_transport = true; } }'})[2]["ok"]},
        {"case": "P04 runner permitted only in runner.rs", "ok": not process_hits(prod_map({GIT_RUNNER: 'Command::new("git")'})) and bool(process_hits(prod_map({a: 'Command::new("git")'})))},
    ]


def check_todo(all_rs, final):
    """todo!/unimplemented!/clippy::todo anywhere in the workspace's .rs files; informational before --final."""
    hits = [f"{r}:{i}" for r, t in sorted(all_rs.items()) for i, l in enumerate(strip_comments(t).splitlines(), 1)
            if re.search(TODO_RE, l)]
    return {"check": "no todo!/unimplemented!/allow(clippy::todo)" + ("" if final else " (informational until --final)"),
            "hits": len(hits) if not final else hits, "ok": (not hits) if final else True}


def check_allows(files, final):
    """#[allow]/#[expect] in src == the approved list. `clippy::todo` entries (B0 stubs carry
    `#[expect(clippy::todo, reason = "B0 stub")]`) are ignored unless final, where they are unapproved."""
    found = set()
    for r, text in files.items():
        if not SRC_PATH_RE.match(r):
            continue
        # T14 review L-6: `#[expect(..)]` and `cfg_attr(.., allow(..))` count as allowances too.
        for m in re.finditer(ALLOW_RE, strip_comments(text, mask_strings=True)):
            for lint in re.findall(r"[\w:]+", m.group(1)):
                if lint != "reason":
                    found.add((r, lint))
    stubs = {f for f in found if f[1] == "clippy::todo"}
    effective = found if final else found - stubs
    # P02-T14: equality, so a stale approval (allow removed from the code) fails too.
    return {"check": "#[allow]/#[expect] in src == the approved list" + ("" if final else " (clippy::todo stubs ignored until --final)"),
            "found": sorted(map(list, found)), "ignored_stubs": [] if final else sorted(map(list, stubs)),
            "unapproved": sorted(map(list, effective - SRC_ALLOWS)), "stale": sorted(map(list, SRC_ALLOWS - effective)),
            "ok": effective == SRC_ALLOWS}


def p03_selftest():
    """Negative and positive controls for the P03 static rules; each runs the same functions as the real scan.
    A `caught` case needs a hit, a `passes` case needs none, so no case can pass on a rule that never or always fires."""
    res = []
    A, CLI, DOM = "crates/application/src/", "crates/cli/src/", "crates/domain/src/"

    def caught(name, fn, files):
        res.append({"case": name, "ok": bool(fn(prod_map(files)))})

    def passes(name, fn, files):
        res.append({"case": name, "ok": not fn(prod_map(files))})

    # --- rule 5: environment reads only in cli/src/main.rs ---
    caught("env: std::env::vars_os() caught outside main.rs", env_hits, {A + "lib.rs": "fn f() { let _ = std::env::vars_os(); }"})
    passes("env: std::env::vars_os() allowed in cli/src/main.rs", env_hits, {CLI_MAIN: "fn f() { let _ = std::env::vars_os(); }"})
    caught("env: std::env::vars() caught", env_hits, {A + "lib.rs": "fn f() { for _ in std::env::vars() {} }"})
    caught("env: env::var_os caught", env_hits, {A + "lib.rs": "fn f() { env::var_os(\"K\"); }"})
    caught("env: use std::env::{var, vars}; caught", env_hits, {CLI + "commands.rs": "use std::env::{var, vars};"})
    caught("env: multi-line brace import with vars_os caught", env_hits, {A + "x.rs": "use std::env::{\n    args,\n    vars_os,\n};"})
    caught("env: nested brace import use std::{env::{self, var}} caught", env_hits, {A + "x.rs": "use std::{env::{self, var}, io};"})
    caught("env: single import use std::env::var; caught", env_hits, {A + "x.rs": "use std::env::var;"})
    caught("env: glob import use std::env::*; caught", env_hits, {A + "x.rs": "use std::env::*;"})
    caught("env: temp_dir in persistence still caught", env_hits, {"crates/persistence/src/paths.rs": "let t = std::env::temp_dir();"})
    caught("env: current_dir in domain now in scope", env_hits, {DOM + "text.rs": "let t = std::env::current_dir();"})
    passes("env: std::env::args() allowed outside main.rs", env_hits, {A + "x.rs": "let a = std::env::args();"})
    passes("env: use std::env::{args, args_os}; allowed", env_hits, {A + "x.rs": "use std::env::{args, args_os};"})
    passes("env: lookup inside #[cfg(test)] mod ignored", env_hits,
           {A + "x.rs": "fn p() {}\n#[cfg(test)]\nmod tests {\n    fn t() { std::env::vars_os(); }\n}\n"})
    passes("env: lookup in a comment ignored", env_hits, {A + "x.rs": "// std::env::vars_os()\n/* env::var(\"X\") */\nfn p() {}"})
    passes("env: crates/*/tests files are out of scope", env_hits, {"crates/cli/tests/p03_x.rs": "std::env::vars_os();"})
    # --- rule 6: no process spawning before P06 ---
    caught("process: std::process::Command::new(\"x\") caught", process_hits, {A + "x.rs": 'fn f() { std::process::Command::new("x"); }'})
    caught("process: use std::process::{Command, ExitCode}; caught", process_hits, {CLI + "x.rs": "use std::process::{Command, ExitCode};"})
    caught("process: bare Command::new( caught", process_hits, {A + "x.rs": "let c = Command::new(prog);"})
    caught("process: use std::process::Command; caught", process_hits, {A + "x.rs": "use std::process::Command;"})
    caught("process: process::Command path caught", process_hits, {A + "x.rs": "let c: process::Command;"})
    caught("process: nested use std::{process::{self, Command}} caught", process_hits, {A + "x.rs": "use std::{process::{self, Command}};"})
    caught("process: glob use std::process::*; caught", process_hits, {A + "x.rs": "use std::process::*;"})
    caught("process: cli/src/main.rs has no exemption", process_hits, {CLI_MAIN: "fn main() { Command::new(\"x\"); }"})
    caught("process: Command::new  (spaced) caught", process_hits, {A + "x.rs": "Command :: new(\"x\")"})
    passes("process: ExitCode::from(2) allowed", process_hits, {CLI_MAIN: "fn main() -> ExitCode { ExitCode::from(2) }"})
    passes("process: use std::process::ExitCode; allowed", process_hits, {CLI_MAIN: "use std::process::ExitCode;"})
    passes("process: use std::process::{self, ExitCode}; allowed", process_hits, {CLI_MAIN: "use std::process::{self, ExitCode};"})
    passes("process: Command inside #[cfg(test)] mod ignored", process_hits,
           {A + "x.rs": "fn p() {}\n#[cfg(test)]\nmod tests {\n    fn t() { std::process::Command::new(\"x\"); }\n}\n"})
    passes("process: Command in a comment ignored", process_hits, {A + "x.rs": "// Command::new(\"x\")\nfn p() {}"})
    passes("process: a type named CommandKind is not Command::new", process_hits, {A + "x.rs": "let k = CommandKind::new();"})
    # --- rule 7: expose allowlist ---
    caught("expose: secret.expose() caught in a non-allowlisted file", expose_hits, {A + "lib.rs": "let b = secret.expose();"})
    caught("expose: caught in cli/src/commands.rs", expose_hits, {CLI + "commands.rs": "s.expose()"})
    passes("expose: allowed in application/src/credential.rs", expose_hits, {A + "credential.rs": "let b = secret.expose();"})
    passes("expose: allowed in domain/src/contract.rs", expose_hits, {DOM + "contract.rs": "pub fn expose(&self) -> &[u8] {}"})
    passes("expose: expose_secret-style names and 'exposed' not matched", expose_hits, {A + "lib.rs": "fn expose_len() {} let exposed = 1;"})
    # --- rule 8: CredentialSecret::new ---
    caught("secret new: CredentialSecret::new(s) caught in cli/src/commands.rs", secret_new_hits, {CLI + "commands.rs": "let k = CredentialSecret::new(s);"})
    caught("secret new: CredentialSecret :: new (spaced) caught", secret_new_hits, {A + "x.rs": "CredentialSecret :: new(s)"})
    caught("secret new: alias import `CredentialSecret as S` caught", secret_new_hits,
           {A + "x.rs": "use domain::contract::CredentialSecret as S;"})
    passes("secret new: allowed in application/src/credential.rs", secret_new_hits, {A + "credential.rs": "CredentialSecret::new(bytes)"})
    passes("secret new: allowed in domain/src/contract.rs", secret_new_hits, {DOM + "contract.rs": "CredentialSecret::new(bytes)"})
    passes("secret new: naming the type in a signature is allowed", secret_new_hits,
           {A + "x.rs": "fn set(&mut self, secret: CredentialSecret) -> CredentialStatus {}"})
    # --- rule 9: serde_json::from_* ---
    caught("serde: from_slice caught in application/src/emit.rs", serde_from_hits, {A + "emit.rs": "serde_json::from_slice(b)"})
    passes("serde: from_slice allowed in application/src/dev_permission.rs", serde_from_hits, {A + "dev_permission.rs": "serde_json::from_slice(b)"})
    passes("serde: from_str allowed in persistence/src/read.rs", serde_from_hits, {"crates/persistence/src/read.rs": "serde_json::from_str(t)"})
    caught("serde: turbofish from_value::<R> caught elsewhere", serde_from_hits, {A + "x.rs": "let r = from_value::<R>(v);"})
    caught("serde: exact path match, a sibling file name is not exempt", serde_from_hits,
           {A + "dev_permission_extra.rs": "serde_json::from_slice(b)"})
    caught("serde: still caught in cli/src", serde_from_hits, {CLI + "commands.rs": "serde_json::from_reader(r)"})
    # --- rule 10: redact_validated ---
    caught("redact_validated: call outside values.rs caught", redact_validated_hits, {A + "x.rs": "let t = redact_validated(x);"})
    caught("redact_validated: call in domain/src/contract.rs caught", redact_validated_hits, {DOM + "contract.rs": "redact_validated (x)"})
    passes("redact_validated: call in domain/src/store/values.rs allowed", redact_validated_hits,
           {DOM + "store/values.rs": "use crate::text::redact_validated;\nlet t = redact_validated(x);"})
    passes("redact_validated: definition in domain/src/text.rs is not a call", redact_validated_hits,
           {DOM + "text.rs": "pub(crate) fn redact_validated(s: &str) -> RedactedText { todo_body }"})
    caught("redact_validated: a call inside text.rs is still caught", redact_validated_hits,
           {DOM + "text.rs": "pub(crate) fn redact_validated(s: &str) -> RedactedText { x }\nfn g() { redact_validated(s) }"})
    caught("redact_validated: a definition in another file is caught", redact_validated_hits,
           {A + "x.rs": "fn redact_validated(s: &str) {}"})
    caught("redact_validated: function-pointer use .map(redact_validated) caught", redact_validated_hits,
           {A + "x.rs": "v.iter().map(redact_validated)"})
    passes("redact_validated: mention in a comment ignored", redact_validated_hits, {A + "x.rs": "// redact_validated(x)\nfn p() {}"})
    # --- rule 11: Authorisation / Grant construction ---
    caught("Authorisation { approval_id } caught outside approvals.rs", authorisation_hits,
           {A + "lib.rs": "let a = Authorisation { approval_id, lifetime };"})
    caught("Authorisation{..} without space caught", authorisation_hits, {CLI + "x.rs": "Authorisation{ approval_id }"})
    caught("Authorisation { .. } caught in a sibling file", authorisation_hits, {A + "dev_permission.rs": "Authorisation { approval_id }"})
    passes("Authorisation { .. } allowed in application/src/approvals.rs", authorisation_hits,
           {A + "approvals.rs": "Ok(AuthOutcome::Authorised(Authorisation { approval_id, lifetime }))"})
    passes("pub struct Authorisation { is a declaration", authorisation_hits, {A + "x.rs": "pub struct Authorisation { approval_id: ApprovalId }"})
    passes("struct Authorisation on two lines is a declaration", authorisation_hits, {A + "x.rs": "pub struct Authorisation\n{\n    a: u8,\n}"})
    passes("impl Authorisation { and impl T for Authorisation { are not literals", authorisation_hits,
           {A + "x.rs": "impl Authorisation { fn id(&self) {} }\nimpl Drop for Authorisation { fn drop(&mut self) {} }"})
    passes("-> Authorisation { return type is not a literal", authorisation_hits, {A + "x.rs": "fn f() -> Authorisation { g() }"})
    passes("AuthorisationKind { .. } is another type", authorisation_hits, {A + "x.rs": "AuthorisationKind { a }"})
    passes("Authorisation { .. } inside #[cfg(test)] mod ignored", authorisation_hits,
           {A + "x.rs": "fn p() {}\n#[cfg(test)]\nmod tests {\n    fn t() { Authorisation { a: 1 }; }\n}\n"})
    caught("Grant { .. } caught in application/src/approvals.rs", grant_hits, {A + "approvals.rs": "Grant { approval_id, lifetime }"})
    caught("Grant { .. } caught outside the application crate", grant_hits, {CLI + "commands.rs": "Evaluation::Permitted(Grant { approval_id })"})
    caught("Grant { .. } caught in a sibling directory permissionx/", grant_hits, {A + "permissionx/x.rs": "Grant { a }"})
    passes("Grant { .. } allowed in permission/evaluate.rs", grant_hits, {A + "permission/evaluate.rs": "Evaluation::Permitted(Grant { approval_id, lifetime })"})
    passes("Grant { .. } allowed in permission/mod.rs", grant_hits, {A + "permission/mod.rs": "Grant { approval_id, lifetime }"})
    passes("pub struct Grant { is a declaration anywhere", grant_hits, {A + "x.rs": "pub struct Grant { approval_id: ApprovalId }"})
    passes("GrantKind { .. } is another type", grant_hits, {A + "x.rs": "GrantKind { a }"})
    # --- rule 12: #[allow]/#[expect] equality, clippy::todo stubs, --final ---
    approved = {}
    for r, lint in sorted(SRC_ALLOWS):
        approved[r] = approved.get(r, "") + f"#[allow({lint})]\nfn f_{len(approved)}() {{}}\n"
    stub = '#[expect(clippy::todo, reason = "B0 stub")]\nfn stub() { todo!() }\n'
    stub_files = {**approved, A + "permission/evaluate.rs": stub}
    res.append({"case": "allows: the approved list alone passes (no --final)", "ok": check_allows(approved, False)["ok"]})
    res.append({"case": "allows: the approved list alone passes (--final)", "ok": check_allows(approved, True)["ok"]})
    res.append({"case": "allows: #[expect(clippy::todo, reason = \"B0 stub\")] accepted without --final",
                "ok": check_allows(stub_files, False)["ok"]})
    res.append({"case": "allows: #[expect(clippy::todo, reason = \"B0 stub\")] rejected with --final",
                "ok": not check_allows(stub_files, True)["ok"]})
    res.append({"case": "allows: --final reports the stub as unapproved",
                "ok": [A + "permission/evaluate.rs", "clippy::todo"] in check_allows(stub_files, True)["unapproved"]})
    res.append({"case": "allows: #[allow(clippy::todo)] spelled as allow is also ignored without --final",
                "ok": check_allows({**approved, A + "x.rs": "#[allow(clippy::todo)]\nfn s() {}\n"}, False)["ok"]})
    res.append({"case": "allows: an unapproved #[allow(dead_code)] fails",
                "ok": not check_allows({**approved, A + "x.rs": "#[allow(dead_code)]\nfn s() {}\n"}, False)["ok"]})
    res.append({"case": "allows: an unapproved #[expect(dead_code)] next to clippy::todo fails",
                "ok": not check_allows({**approved, A + "x.rs": '#[expect(clippy::todo, dead_code, reason = "B0 stub")]\nfn s() {}\n'}, False)["ok"]})
    res.append({"case": "allows: a stale approval (allow removed) fails",
                "ok": not check_allows({r: t for r, t in approved.items() if r != "crates/domain/src/text.rs"}, False)["ok"]})
    res.append({"case": "allows: an allow inside #[cfg(test)] mod still counts",
                "ok": not check_allows({**approved, A + "x.rs": "#[cfg(test)]\nmod tests {\n    #[allow(dead_code)]\n    fn t() {}\n}\n"}, False)["ok"]})
    res.append({"case": "allows: files outside crates/*/src are not scanned",
                "ok": check_allows({**approved, "crates/cli/tests/p03_x.rs": "#[allow(dead_code)]\nfn t() {}\n"}, False)["ok"]})
    res.append({"case": "todo rule: informational without --final", "ok": check_todo({A + "x.rs": stub}, False)["ok"]})
    res.append({"case": "todo rule: todo!/clippy::todo fails with --final", "ok": not check_todo({A + "x.rs": stub}, True)["ok"]})
    res.append({"case": "todo rule: clean tree passes with --final", "ok": check_todo({A + "x.rs": "fn ok() {}\n"}, True)["ok"]})
    res.append({"case": "todo rule: todo! in a comment is ignored with --final",
                "ok": check_todo({A + "x.rs": "// todo!()\nfn ok() {}\n"}, True)["ok"]})
    # --- rule 12 (T11, D01 H1): Evaluation::Permitted and permission::evaluate only in approvals.rs ---
    caught("Permitted: match on Evaluation::Permitted caught in dev_permission.rs", permitted_hits,
           {A + "dev_permission.rs": "match e { Evaluation::Permitted(g) => run(g), _ => {} }"})
    caught("Permitted: Evaluation :: Permitted with spaces caught in cli", permitted_hits,
           {CLI + "commands.rs": "if let Evaluation :: Permitted(_) = e {}"})
    caught("Permitted: use Evaluation::{Permitted, ..} caught", permitted_hits,
           {A + "lib.rs": "use crate::permission::Evaluation::{NeedsApproval, Permitted};"})
    caught("Permitted: use Evaluation::* caught", permitted_hits, {A + "lib.rs": "use crate::permission::Evaluation::*;"})
    passes("Permitted: allowed in application/src/approvals.rs", permitted_hits,
           {A + "approvals.rs": "match e { Evaluation::Permitted(g) => g, _ => todo() }"})
    passes("Permitted: allowed under application/src/permission/", permitted_hits,
           {A + "permission/evaluate.rs": "Evaluation::Permitted(Grant { a })"})
    passes("Permitted: RequestOutcome::Permitted is another enum", permitted_hits,
           {CLI + "commands.rs": "RequestOutcome::Permitted => 0, AuthOutcome::Authorised(_) => 0"})
    passes("Permitted: Evaluation::Permitted in a #[cfg(test)] module ignored", permitted_hits,
           {A + "x.rs": "fn p() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = Evaluation::Permitted; }\n}\n"})
    caught("evaluate: permission::evaluate call caught in dev_permission.rs", evaluate_hits,
           {A + "dev_permission.rs": "let e = crate::permission::evaluate(&r, &a, &c);"})
    caught("evaluate: use permission::{evaluate, ..} caught across lines", evaluate_hits,
           {A + "dev_permission.rs": "use crate::permission::{\n    PermissionRequest,\n    evaluate,\n};"})
    caught("evaluate: use permission::* caught", evaluate_hits, {CLI + "lib.rs": "use application::permission::*;"})
    passes("evaluate: allowed in approvals.rs", evaluate_hits,
           {A + "approvals.rs": "use crate::permission::{EvaluationContext, evaluate, fingerprint};"})
    passes("evaluate: permission::{PermissionRequest, fingerprint} without evaluate passes", evaluate_hits,
           {A + "dev_permission.rs": "use crate::permission::{PermissionRequest, fingerprint};"})
    passes("evaluate: evaluate_due or other names are not permission::evaluate", evaluate_hits,
           {A + "x.rs": "fn f() { scheduler::evaluate_due(); permission::evaluated_at(); }"})
    # --- rule 13 (P03-D02 C1): RequestOutcome::Permitted only in approvals.rs ---
    caught("RequestOutcome::Permitted caught in cli", request_permitted_hits,
           {CLI + "commands.rs": "match o { RequestOutcome::Permitted => go(), _ => {} }"})
    caught("RequestOutcome :: Permitted with spaces caught", request_permitted_hits,
           {A + "dev_permission.rs": "if let RequestOutcome :: Permitted = o {}"})
    caught("use RequestOutcome::{Permitted, ..} caught", request_permitted_hits,
           {A + "lib.rs": "use crate::approvals::RequestOutcome::{Pending, Permitted};"})
    caught("use RequestOutcome::* caught", request_permitted_hits, {CLI + "lib.rs": "use application::approvals::RequestOutcome::*;"})
    passes("RequestOutcome::Permitted allowed in approvals.rs", request_permitted_hits,
           {A + "approvals.rs": "Evaluation::Permitted(_) => Ok(RequestOutcome::Permitted),"})
    passes("RequestOutcome::Pending elsewhere passes", request_permitted_hits,
           {CLI + "commands.rs": "RequestOutcome::Pending(id) => id, AuthOutcome::Authorised(a) => a"})
    passes("RequestOutcome::Permitted in a #[cfg(test)] module ignored", request_permitted_hits,
           {CLI + "x.rs": "fn p() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = RequestOutcome::Permitted; }\n}\n"})
    # --- check_p03 wiring: every rule reports a check entry and the clean map passes all ---
    clean = check_p03(prod_map({A + "lib.rs": "pub fn ok() {}\n"}))
    res.append({"case": "check_p03: clean map passes all rules and yields 11 checks", "ok": len(clean) == 11 and all(c["ok"] for c in clean)})
    dirty = check_p03(prod_map({A + "lib.rs": "std::env::vars_os(); Command::new(x); s.expose(); CredentialSecret::new(s); "
                                              "serde_json::from_slice(b); redact_validated(x); Authorisation { a }; Grant { a }; "
                                              "Evaluation::Permitted(g); permission::evaluate(r); RequestOutcome::Permitted"}))
    res.append({"case": "check_p03: one file violating every rule fails all 11 checks", "ok": len(dirty) == 11 and not any(c["ok"] for c in dirty)})
    return res


def test_counts(log):
    tot = {"suites": 0, "passed": 0, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 0}
    for m in TEST_RESULT.finditer(log.read_text(errors="replace")):
        tot["suites"] += 1
        for k, v in zip(("passed", "failed", "ignored", "measured", "filtered_out"), m.groups()[1:]):
            tot[k] += int(v)
    return tot


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["env", "fmt", "fmt-fix", "clippy", "test", "build", "static", "cli", "all"])
    ap.add_argument("--slot", required=True, choices=SLOTS)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--package")
    ap.add_argument("--final", action="store_true")
    ap.add_argument("--min-passed", type=int, default=0, help="fail the test step below this many passed tests")
    ap.add_argument("--min-compile-fail", type=int, default=0, help="fail the test step below this many compile_fail doctests")
    ap.add_argument("--repo", type=Path, help="checkout to verify (default: this script's checkout)")
    a = ap.parse_args()
    global REPO, WS
    if a.repo:
        REPO = a.repo.resolve()
        WS = REPO / "workbench"
    if not (WS / "Cargo.toml").is_file():
        print(f"no workspace at {WS}")
        return 2
    if a.final:
        base = json.loads((Path(__file__).resolve().parent / "p04_baseline.json").read_text())
        a.min_passed = max(a.min_passed, int(base["min_passed"]))
        a.min_compile_fail = max(a.min_compile_fail, int(base["min_compile_fail"]))
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scope = ["-p", a.package] if a.package else ["--workspace"]
    modes = ["env", "fmt", "clippy", "test", "build", "cli", "static"] if a.mode == "all" else [a.mode]
    if a.mode not in ("env", "all", "static"):
        modes = ["env", *modes]  # T01 §2: version gate and netns probe before any Cargo command
    # T14 security review L-4: repository-local git config (fsmonitor, hooks) is not trusted for --repo trees.
    git = ["git", "--no-optional-locks", "-c", "core.fsmonitor=", "-c", "core.hooksPath=/dev/null", "-C", str(REPO)]
    genv = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_NOSYSTEM": "1", "LC_ALL": "C"}
    head = subprocess.run([*git, "rev-parse", "HEAD"], capture_output=True, text=True, env=genv).stdout.strip()
    dirty = subprocess.run([*git, "status", "--short"], capture_output=True, text=True, env=genv).stdout
    results = []
    for m in modes:
        if m == "env":
            r = step_env(a.slot, out)
        elif m == "fmt":
            r = run_cargo("fmt", ["fmt", "--all", "--", "--check"], a.slot, out)
        elif m == "fmt-fix":
            # Applies rustfmt in place (same pinned toolchain, env -i, netns). Not part of `all`: a check
            # run must never rewrite sources. Replaces ad-hoc `unshare … cargo fmt` invocations.
            r = run_cargo("fmt-fix", ["fmt", "--all"], a.slot, out)
        elif m == "clippy":
            r = run_cargo("clippy", ["clippy", *scope, "--all-targets", "--frozen", "--", "-D", "warnings"], a.slot, out)
        elif m == "test":
            # --no-fail-fast (P02-D06): a failing test binary must not hide the results of the later ones.
            r = run_cargo("test", ["test", *scope, "--frozen", "--no-fail-fast"], a.slot, out)
            r["counts"] = test_counts(out / "test.log")
            log_text = (out / "test.log").read_text(errors="replace")
            r["counts"]["compile_fail_ok"] = len(re.findall(r" - compile fail \.\.\. ok$", log_text, re.M))
            r["floors"] = {"min_passed": a.min_passed, "min_compile_fail": a.min_compile_fail}
            if (r["counts"]["ignored"] or r["counts"]["failed"] or not r["counts"]["suites"]
                    or r["counts"]["passed"] < a.min_passed or r["counts"]["compile_fail_ok"] < a.min_compile_fail):
                r["ok"] = False
        elif m == "cli":
            r = run_cargo("cli", ["test", "-p", "cli", "--frozen", "--tests", "--no-fail-fast"], a.slot, out)
            r["counts"] = test_counts(out / "cli.log")
            if r["counts"]["failed"] or r["counts"]["ignored"]:
                r["ok"] = False
        elif m == "build":
            r = run_cargo("build", ["build", *scope, "--frozen"], a.slot, out)
        else:
            r = step_static(a.final, out)
        results.append(r)
        if m == "env" and not r["ok"]:
            break
    summary = {"tool": "dev/p04_check.py", "repo": str(REPO), "target_dir": str(target_dir(a.slot)), "mode": a.mode, "slot": a.slot, "package": a.package, "final": a.final,
               "repo_head": head, "dirty": dirty.splitlines(), "results": results,
               "ok": all(r["ok"] for r in results) and len(results) == len(modes)}
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for r in results:
        extra = f" counts={r['counts']}" if "counts" in r else ""
        print(f"{'PASS' if r['ok'] else 'FAIL'} {r['step']} exit={r.get('exit', '-')}{extra}")
    print("OVERALL", "PASS" if summary["ok"] else "FAIL")
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
