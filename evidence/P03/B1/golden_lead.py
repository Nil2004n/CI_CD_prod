"""Lead's independent re-computation of the P03 fingerprint golden vectors (request §5.3).
Written from the spec text; inputs are the Rust test's raw inputs (Rust variant names, raw destination
URLs). Canonicalisation (request §6) and snake_case naming are done here, not copied."""
import hashlib, re, struct
from urllib.parse import urlsplit

PREFIX = b"pwt-operation-fingerprint\x00v1\x00"
DEFAULT_PORT = {"https": 443, "ssh": 22}

def snake(variant): return re.sub(r"(?<!^)(?=[A-Z])", "_", variant).lower()
def enc_str(s): b = s.encode(); return struct.pack(">I", len(b)) + b
def enc_list(items):
    items = sorted(set(items)); return struct.pack(">I", len(items)) + b"".join(items)
def canonical(url):
    u = urlsplit(url); host = u.hostname.lower().removesuffix(".")
    port = u.port or DEFAULT_PORT[u.scheme]; path = u.path if u.path not in ("", "/") else ""
    return f"{u.scheme}://{host}:{port}{path}"

def fp(cap, op, args=(), repos=(), dests=(), cons=(), data=()):
    b = PREFIX + enc_str(snake(cap)) + enc_str(op)
    b += enc_list([enc_str(n) + enc_str(v) for n, v in args])
    b += enc_list([enc_str(r) for r in repos])
    b += enc_list([enc_str(canonical(d)) for d in dests])
    b += enc_list([enc_str(c) for c in cons])
    b += enc_list([enc_str("raw_secret_values" if d == "RAW" else snake(d)) for d in data])
    return hashlib.sha256(b).hexdigest()

R1, R2, R3 = "00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002", "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b"
ALL = ["RepositoryMetadata", "RedactedSource", "RedactedFindings", "SchemaDocuments", "PublicArtifactIdentifiers", "RedactedChatContext", "AppMetadata"]
V = {
 "V1": (fp("GitAcquire", "git.clone", [("branch", "main"), ("depth", "1")], [R1], ["https://github.com/example/repo"], ["max_size_mb=100"], ["RepositoryMetadata"]), "a602941b5c4d15d92b28ea5f4c305b526947fda4649176330546c69537ae9e5c"),
 "V2": (fp("LocalRender", "render.template"), "02000fa2adccc8bb04e1627fc18ef56d7eaf1b3621844b096549c893463e855b"),
 "V3": (fp("ModelTransmission", "chat.send", [("model", "openai/gpt-4o:free")], [R2, R1], ["https://openrouter.ai/api/v1"], ["max_tokens=2048", "max_requests=1"], ["RedactedSource", "RedactedFindings", "RedactedSource"]), "4508c1fd50e8a08e3742499c424f6b40290dd4fd42e003e397e3a26f5d6cecea"),
 "V4": (fp("GitUpdate", "git.fetch", [("remote", "origin"), ("prune", "true"), ("ref", "refs/heads/main")], [R3, R1, R2], ["ssh://git.example.org:2222/team/repo.git", "https://example.com"], [], ["AppMetadata", "PublicArtifactIdentifiers"]), "8f9b9dfbcaffd3766f189dc70051779ae88c8c543eaa28ac16ed82b638ac2dd4"),
 "V5": (fp("SchemaFetch", "schema.fetch", [("title", "zażółć gęślą"), ("empty", "")], [], ["https://schemas.example.com/v1/schema.json"], ["café limit"], ["SchemaDocuments"]), "dd9f64515a83da293d96c34e893c3f88e2e6eb9e39493ae8ebee366af00dd2bd"),
 "V6": (fp("ExternalReference", "ref.fetch", [], [R1], ["https://example.com:8443/docs"], [], ["RAW", "RedactedChatContext"]), "cc9db5e3830962b2f0f2c4cc64bdff845208180d36ac40934f2ec902280e08d9"),
 "V7": (fp("VulnerabilityLookup", "vuln.query", [("package", "left-pad"), ("version", "1.3.0")], [R1], ["https://api.osv.dev/v1/query"], ["rate=low"], ALL), "a3eec4c5cf42f3a890d03dc07f65971ca8275df2c2b57e50bc4b8c9e8166de8c"),
}
bad = 0
for k, (got, pinned) in V.items():
    ok = got == pinned; bad += not ok; print(k, "MATCH" if ok else f"MISMATCH lead={got}")
print("golden vectors:", "PASS" if not bad else f"FAIL ({bad})"); raise SystemExit(bad)
