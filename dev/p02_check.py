"""P02 development check runner (planning/P02-T01-request.md §8; derived from dev/p01_check.py, which stays
unchanged as the P01 record). Development only; not shipped.

Usage: python3 -I -B dev/p02_check.py <mode> --slot <lead|w1|w2|w3> --out <dir> [--package <crate>] [--final] [--repo <checkout>]
Modes: env, fmt, clippy, test, build, static, cli, all (= env fmt clippy test build cli static).
`cli` runs every scripted CLI scenario (`cargo test -p cli --tests`), headless.

Every Cargo command runs the 1.97.1 toolchain binaries directly, with `env -i` and the isolated
CARGO_HOME, inside `unshare -rn`, with --frozen (fmt has no network use). Exit codes are captured
from the producer process itself. Cargo.lock and every Cargo.toml are hashed before and after each step.
"""

import argparse
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
PWT = Path("/home/kali/.local/share/pwt-dev-tools")
TC = Path("/home/kali/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin")
TOOLCHAIN_VERSION = "1.97.1"
SLOTS = ("lead", "w1", "w2", "w3")
# Approved registry set (version, source, checksum), every entry equal to the P00 lock entry
# (evidence/P02/T02/record.md); keyed "name version" because syn appears in two versions.
REGISTRY_FILE = Path(__file__).resolve().parent / "p02_registry.json"
EXPECTED_REGISTRY = {k: tuple(v) for k, v in json.loads(REGISTRY_FILE.read_text())["registry"].items()}
MEMBERS = {"domain", "application", "repositories", "scanning", "retrieval", "persistence", "terminal-ui", "cli"}
ALLOWED_DIRECT_DEPS = {"serde", "serde_json", "jsonschema", "sha2", "domain", "persistence", "application"}
TEST_RESULT = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out")


def target_dir(slot):
    """Per slot AND per repository checkout. Cargo's artifact names for workspace members do not depend on
    the workspace's absolute path, so two identically laid-out worktrees sharing one target directory can
    link each other's stale builds (found in P01 wave-1 verification, evidence/P01/T07/lead-verify)."""
    return PWT / "_cargo-target" / f"p02-{slot}-{hashlib.sha256(str(REPO).encode()).hexdigest()[:12]}"


def cargo_env(slot):
    return [
        f"PATH={TC}:/usr/bin:/bin",
        f"HOME={PWT}/_cargo-home",
        f"CARGO_HOME={PWT}/_cargo-home",
        f"CARGO_TARGET_DIR={target_dir(slot)}",
        f"RUSTC={TC}/rustc",
        f"RUSTDOC={TC}/rustdoc",
        "CARGO_TERM_COLOR=never",
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
    probe = ("import socket\ns=socket.socket();s.settimeout(3)\n"
             "try:\n s.connect(('1.1.1.1',443));print('REACHABLE')\nexcept OSError as e:\n print('errno',e.errno)")
    p = subprocess.run(["unshare", "-rn", sys.executable, "-I", "-c", probe], capture_output=True, text=True, check=False)
    checks.append({"check": "netns outbound connect fails", "value": p.stdout.strip(),
                   "ok": p.returncode == 0 and p.stdout.startswith("errno")})
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


# Lock file hash resolved and accepted in P02-T02 (evidence/P02/T02/record.md).
EXPECTED_LOCK_SHA256 = "b34cb529a79905546621573fef5de55b2be29884b2028715ef67e68bf0ef1c26"
# The only `#[allow(...)]` attributes permitted in production code (file, lint) — planning/P01-T01-request.md §6.1.
SRC_ALLOWS = {("crates/domain/src/contract.rs", "clippy::result_large_err"),
              # Temporary (P02 B0 -> B1): store_read helpers unused until the wave-1b evidence-side models.
              ("crates/domain/src/text.rs", "dead_code")}
# The only production call sites of `redact_static` (file); argument must be a literal or ALL_CAPS const.
REDACT_STATIC_CALLERS = {"crates/domain/src/contract.rs"}
EXPOSE_EXEMPT = "crates/domain/src/contract.rs"
REDACTOR_IMPL_RE = r"\bimpl\b[^{;]*?\bRedactor\b[^{;]*?\bfor\b"

PRINT_RE = r"\b(e?print(ln)?|dbg)\s*!|\bio::std(out|err)\s*\(|\bstd(out|err)\s*\(\s*\)"
PRINT_MACRO_RE = r"\b(e?print(ln)?|dbg)\s*!"
# The binary entry point is the only production file that may take the process stdout/stderr handles.
CLI_MAIN = "crates/cli/src/main.rs"


def step_static(final, out):
    checks = []
    rs = sorted(WS.rglob("*.rs"))
    src = [p for p in rs if (WS / "crates") in p.parents and p.relative_to(WS / "crates").parts[1:2] == ("src",)]
    hits = [f"{p.relative_to(REPO)}:{i}" for p in rs for i, l in code_lines(p) if re.search(r"\bunsafe\b", l)]
    checks.append({"check": "no `unsafe` in code (comments stripped)", "hits": hits, "ok": not hits})
    hits = [f"{p.relative_to(REPO)}:{i}" for p in src for i, l in code_lines(p, production=True)
            if re.search(PRINT_MACRO_RE if p == WS / CLI_MAIN else PRINT_RE, l)]
    checks.append({"check": "no print/dbg/stdout/stderr in src", "hits": hits, "ok": not hits})
    hits = [f"{p.relative_to(REPO)}:{i}" for p in rs for i, l in code_lines(p)
            if re.search(r"\btodo\s*!|\bunimplemented\s*!|clippy::todo", l)]
    checks.append({"check": "no todo!/unimplemented!/allow(clippy::todo)" + ("" if final else " (informational before T09)"),
                   "hits": len(hits), "ok": (not hits) if final else True})
    def rel(p):
        return str(p.relative_to(WS))

    prod = {p: drop_test_modules(strip_comments(p.read_text())) for p in src}

    def line_of(code, pos):
        return code.count("\n", 0, pos) + 1

    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p) != EXPOSE_EXEMPT
            for m in re.finditer(r"\bexpose\b(?!_)", c)]
    checks.append({"check": "CredentialSecret::expose not referenced in src outside domain/src/contract.rs", "hits": hits, "ok": not hits})
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
    found = set()
    for p, c in prod.items():
        for m in re.finditer(r"#!?\s*\[\s*allow\s*\(([^)]*)", strip_comments(p.read_text(), mask_strings=True)):
            for lint in re.findall(r"[\w:]+", m.group(1)):
                if lint != "reason":
                    found.add((rel(p), lint))
    checks.append({"check": "#[allow] in src limited to the approved list", "found": sorted(map(list, found)),
                   "unapproved": sorted(map(list, found - SRC_ALLOWS)), "ok": found <= SRC_ALLOWS})
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
    st = static_selftest() + p02_selftest()
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
    checks.append({"check": "Cargo.lock sha256 == T02-accepted lock", "value": lock_sha, "ok": lock_sha == EXPECTED_LOCK_SHA256})
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
SERDE_FROM_OK = {"crates/persistence/src/read.rs", "crates/persistence/src/schema.rs"}
FS_RE = r"\bstd\s*::\s*fs\b|\bfs\s*::\s*(File|OpenOptions|read|write|rename|remove_file|create_dir|hard_link|metadata|symlink_metadata|read_dir|copy)\b|\bFile\s*::|\bOpenOptions\b"
FS_OK = {"crates/persistence/src/fs.rs", "crates/application/src/emit.rs"}
ENV_RE = r"\benv\s*::\s*(var|var_os|vars|vars_os|home_dir|temp_dir|current_dir)\b|\bhome_dir\s*\("
ENV_SCOPE = ("crates/cli/src/", "crates/application/src/", "crates/persistence/src/")


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
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p) not in SERDE_FROM_OK
            for m in re.finditer(SERDE_FROM_RE, c)]
    checks.append({"check": "serde_json::from_* only in persistence/src/{read,schema}.rs", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p) not in FS_OK
            for m in re.finditer(FS_RE, c)]
    checks.append({"check": "std::fs / File / OpenOptions only in persistence/src/fs.rs and application/src/emit.rs", "hits": hits, "ok": not hits})
    hits = [f"{rel(p)}:{line_of(c, m.start())}" for p, c in prod.items() if rel(p).startswith(ENV_SCOPE)
            for m in re.finditer(ENV_RE, c)]
    checks.append({"check": "no environment/home/temp/cwd lookups in cli, application, persistence src", "hits": hits, "ok": not hits})
    return checks


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
    for s in ("std::fs::read(p)", "use std::fs;", "File::open(p)", "OpenOptions::new()", "fs::rename(a, b)"):
        res.append({"case": f"fs use caught: {s}", "ok": bool(re.search(FS_RE, s))})
    res.append({"case": "fs regex ignores FsOps trait use", "ok": not re.search(FS_RE, "fs.rename(&a, &b)?; let f: &dyn FsOps = x;")})
    for s in ("std::env::var(\"HOME\")", "env::var_os(k)", "std::env::temp_dir()", "env::home_dir()", "std::env::current_dir()"):
        res.append({"case": f"env lookup caught: {s}", "ok": bool(re.search(ENV_RE, s))})
    res.append({"case": "env regex allows args_os", "ok": not re.search(ENV_RE, "std::env::args_os()")})
    res.append({"case": "print macro regex catches println in main", "ok": bool(re.search(PRINT_MACRO_RE, 'println!("x")'))})
    res.append({"case": "print macro regex allows stdout handle in main", "ok": not re.search(PRINT_MACRO_RE, "std::io::stdout().lock()")})
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
    ap.add_argument("mode", choices=["env", "fmt", "clippy", "test", "build", "static", "cli", "all"])
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
        base = json.loads((Path(__file__).resolve().parent / "p02_baseline.json").read_text())
        a.min_passed = max(a.min_passed, int(base["min_passed"]))
        a.min_compile_fail = max(a.min_compile_fail, int(base["min_compile_fail"]))
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scope = ["-p", a.package] if a.package else ["--workspace"]
    modes = ["env", "fmt", "clippy", "test", "build", "cli", "static"] if a.mode == "all" else [a.mode]
    if a.mode not in ("env", "all", "static"):
        modes = ["env", *modes]  # T01 §2: version gate and netns probe before any Cargo command
    head = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "-C", str(REPO), "status", "--short"], capture_output=True, text=True).stdout
    results = []
    for m in modes:
        if m == "env":
            r = step_env(a.slot, out)
        elif m == "fmt":
            r = run_cargo("fmt", ["fmt", "--all", "--", "--check"], a.slot, out)
        elif m == "clippy":
            r = run_cargo("clippy", ["clippy", *scope, "--all-targets", "--frozen", "--", "-D", "warnings"], a.slot, out)
        elif m == "test":
            r = run_cargo("test", ["test", *scope, "--frozen"], a.slot, out)
            r["counts"] = test_counts(out / "test.log")
            log_text = (out / "test.log").read_text(errors="replace")
            r["counts"]["compile_fail_ok"] = len(re.findall(r" - compile fail \.\.\. ok$", log_text, re.M))
            r["floors"] = {"min_passed": a.min_passed, "min_compile_fail": a.min_compile_fail}
            if (r["counts"]["ignored"] or r["counts"]["failed"] or not r["counts"]["suites"]
                    or r["counts"]["passed"] < a.min_passed or r["counts"]["compile_fail_ok"] < a.min_compile_fail):
                r["ok"] = False
        elif m == "cli":
            r = run_cargo("cli", ["test", "-p", "cli", "--frozen", "--tests"], a.slot, out)
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
    summary = {"tool": "dev/p02_check.py", "repo": str(REPO), "target_dir": str(target_dir(a.slot)), "mode": a.mode, "slot": a.slot, "package": a.package, "final": a.final,
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
