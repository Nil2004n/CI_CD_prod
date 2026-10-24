"""P00-T09 shared synthetic fixtures (Opus-owned; P00-T01 fixture specification).

Fake secrets are generated in memory at run time (never committed). Each embeds the marker
`PWTFAKE<canary-id>` where the format allows it, so leak checks can match by marker as well as
by value. No vendor example keys are used. Nothing here executes fixture content.
"""

from __future__ import annotations

import base64
import secrets
import string
from pathlib import Path

ALNUM = string.ascii_letters + string.digits
B32 = string.ascii_uppercase + "234567"


def _r(n: int, alphabet: str = ALNUM) -> str:
    return "".join(secrets.choice(alphabet) for _ in range(n))


def gen_canaries() -> dict[str, str]:
    """Canary ID -> fake value. Values exist only in memory and in scratch fixtures."""
    pat = lambda cid: "ghp_" + f"PWTFAKE{cid}" + _r(36 - len(f"PWTFAKE{cid}"))
    c = {
        "C01": pat("C01"),                                    # in malformed YAML (SCAN-08)
        "C02": pat("C02"),                                    # provider-style prefixed token
        # AWS-like access key id: AKIA + 16 of [A-Z2-7]; marker uses letters only ("PWTFAKECD").
        "C03": "AKIA" + "PWTFAKECD" + _r(7, B32),
        "C04": f"PWTFAKEC04" + _r(30, ALNUM + "/+"),          # AWS-like secret access key (40)
        "C05": f"PWTFAKEC05" + _r(54, ALNUM + "+/"),          # private-key body fragment
        "C06": f"PWTFAKEC06" + _r(14),                        # URL password
        "C07": f"PWTFAKEC07" + _r(24),                        # generic api_key value
        "C08": pat("C08"),                                    # inside a block scalar
        "C09": pat("C09"),                                    # base64-encoded in a K8s Secret
        "C11": pat("C11"),                                    # unselected sibling (scope)
        "C12": pat("C12"),                                    # unselected non-YAML file (scope)
    }
    return c


def canary_markers(c: dict[str, str]) -> dict[str, str]:
    """Canary ID -> the marker actually embedded in its value. C03 embeds `PWTFAKECD` (an AKIA-style id
    allows only [A-Z2-7]), all others `PWTFAKE<ID>`. Detection only; the canaries are unchanged."""
    m = {cid: ("PWTFAKECD" if cid == "C03" else f"PWTFAKE{cid}") for cid in c}
    for cid, mk in m.items():
        assert mk in c[cid], cid
    return m


def pem_block(c: dict[str, str]) -> str:
    body = c["C05"] + _r(64 * 6, ALNUM + "+/")
    lines = [body[i:i + 64] for i in range(0, len(body), 64)]
    head = "-----BEGIN " + "RSA PRIVATE" + " KEY-----"
    tail = "-----END " + "RSA PRIVATE" + " KEY-----"
    return "\n".join([head, *lines, tail]) + "\n"


def write_tree(dest: Path, files: dict[str, bytes | str]) -> None:
    for rel, data in files.items():
        p = dest / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data.encode() if isinstance(data, str) else data)


# ---------------------------------------------------------------- FX-YAML-*
YAML_VALID = "---\napp:\n  name: demo\n  replicas: 2\n  ports:\n    - 80\n    - 443\n"
YAML_STYLE = "---\napp:\n    name: demo   \n    description: " + "x" * 150 + "\n    list: [1,2,  3]\n"
YAML_MALFORMED = "---\napp:\n  name: demo\n  ports: [80, 443\n bad_indent: true\n"
YAML_DUPKEY = "---\nkey: 1\nkey: 2\nnested:\n  a: 1\n  a: 2\n"
YAML_MULTIDOC = "---\na: 1\n---\nb: [unclosed\n---\nc: 3\n"


def yaml_malformed_secret(c):
    return f"---\napp:\n  token: {c['C01']}\n  ports: [80, 443\n bad: true\n"


def secrets_tree(c) -> dict[str, str]:
    """FX-SEC-* (one file per shape)."""
    return {
        "sec/token.yaml": f"---\ngithub:\n  token: {c['C02']}\n",
        "sec/keypair.yaml": f"---\naws:\n  access_key_id: {c['C03']}\n  secret_access_key: {c['C04']}\n",
        "sec/private-key.yaml": "---\ntls:\n  key: |\n" + "".join("    " + l + "\n" for l in pem_block(c).splitlines()),
        "sec/url-cred.yaml": f"---\nrepo:\n  url: https://pwtuser:{c['C06']}@example.invalid/x.git\n",
        "sec/generic.yaml": f"---\nservice:\n  api_key: \"{c['C07']}\"\n",
        "sec/multiline.yaml": f"---\nnotes: |\n  line one\n  token {c['C08']} inside text\n  line three\n",
        "sec/k8s-secret.yaml": "---\napiVersion: v1\nkind: Secret\nmetadata:\n  name: s\ndata:\n  token: "
                               + base64.b64encode(c["C09"].encode()).decode() + "\n",
        "sec/near-miss.yaml": "---\npassword: changeme\napi_key: ${API_KEY}\ntoken: xxxxxxxxxxxxxxxxxxxxxxxx\n",
    }


# ---------------------------------------------------------------- FX-NAME-*
HOSTILE_NAMES = {
    "FX-NAME-DASH-1": "-rf.yaml",
    "FX-NAME-DASH-2": "--config=evil.yaml",
    "FX-NAME-ANSI": "\x1b[31mred\x1b[0m.yaml",
    "FX-NAME-NEWLINE": "line\nbreak.yaml",
    "FX-NAME-GLOB": "* ?[a] $HOME 'q\".yaml",
    "FX-NAME-UNICODE": "‮eviĺ.yaml",
    "FX-NAME-LONG": ("n" * 200) + ".yaml",
}


def hostile_name_tree(content: str = YAML_STYLE) -> dict[str, str]:
    return {f"names/{n}": content for n in HOSTILE_NAMES.values()}


# ---------------------------------------------------------------- FX-SCOPE-*
def scope_tree(c) -> dict[str, str]:
    return {
        "scope/selected.yaml": YAML_STYLE,
        "scope/sibling.yaml": f"---\ntoken: {c['C11']}\n",
        "scope/notes.txt": f"token={c['C12']}\n",
        "scope/sub/nested.yaml": YAML_VALID,
    }


# ---------------------------------------------------------------- FX-GHA-*
GHA_VALID = """name: ci
on: [push]
permissions:
  contents: read
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@0123456789abcdef0123456789abcdef01234567
      - run: echo hello
"""
GHA_INVALID = """name: bad
on: [push]
jobs:
  build:
    steps:
      - run: echo ${{ github.event.foo.
    unknown_key: 1
"""


def gha_injection(marker_dir: Path) -> str:
    return f"""name: inj
on: [pull_request_target]
permissions: write-all
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{{{ github.event.pull_request.head.sha }}}}
      - run: echo "${{{{ github.event.pull_request.title }}}}"
      - run: touch {marker_dir}/PWT_EXECUTED_FX-GHA-INJECTION
"""


GHA_REF = """name: refs
on: [push]
jobs:
  a:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v4
      - uses: someorg/someaction@main
      - uses: actions/cache@0123456789abcdef0123456789abcdef01234567
      - uses: ./.github/actions/local
      - uses: docker://alpine:latest
  b:
    uses: ./.github/workflows/reusable.yml
"""

# ---------------------------------------------------------------- FX-K8S-*
K8S_VALID = """apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
spec:
  replicas: 1
  selector:
    matchLabels: {app: web}
  template:
    metadata:
      labels: {app: web}
    spec:
      containers:
        - name: web
          image: registry.example.invalid/web@sha256:0000000000000000000000000000000000000000000000000000000000000000
          resources:
            limits: {cpu: 100m, memory: 64Mi}
          securityContext:
            runAsNonRoot: true
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
"""
K8S_INSECURE = """apiVersion: apps/v1
kind: Deployment
metadata:
  name: insecure
spec:
  replicas: 1
  selector:
    matchLabels: {app: insecure}
  template:
    metadata:
      labels: {app: insecure}
    spec:
      hostNetwork: true
      containers:
        - name: c
          image: nginx:latest
          securityContext:
            privileged: true
"""
K8S_INVALID = """apiVersion: apps/v1
kind: Deployment
metadata:
  name: bad
spec:
  replicas: "two"
  bogusField: 1
  selector:
    matchLabels: {app: bad}
  template:
    metadata:
      labels: {app: bad}
    spec:
      containers:
        - name: c
          image: nginx
"""
K8S_MULTIDOC = K8S_VALID + "---\n" + K8S_INVALID
K8S_CRD = """apiVersion: example.invalid/v1
kind: Widget
metadata:
  name: w
spec:
  size: 3
"""
K8S_TEMPLATE = """apiVersion: apps/v1
kind: Deployment
metadata:
  name: {{ .Values.name }}
spec:
  replicas: {{ .Values.replicas }}
"""
