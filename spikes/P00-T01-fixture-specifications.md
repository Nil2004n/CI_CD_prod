# P00-T01 — Synthetic fixture specifications

Task: P00-T01 (owner-approved 2026-10-24). Plan reference: `plan.md` §5. Base revision `1e4a5b3`.

Status: **specification only.** No fixture file, generator or repository has been created. Fixtures are built later (spike tasks T04–T09 for spike use; P01+ for the permanent test corpus under `tests/fixtures/`), each with its own approval. Expected outcomes below are **product** expectations from the specifications, not predictions of tool output; tool behaviour is what the spikes measure.

## 1. Global rules

1. **Synthetic only.** No client data, real repositories, real credentials or copied third-party configuration.
2. **Fake secrets are generated, not committed.** The repository stores only a generator specification (shape, canary ID, seed). Values are produced at fixture-build time into a scratch directory or a git-ignored path. Reasons: secret material must not enter evidence or Git history, and the hosting provider's secret scanning/push protection may block or flag pushes containing realistic tokens.
3. **Canary format.** Each fake secret embeds a canary marker `PWTFAKE` plus a canary ID (e.g. `C01`) inside the value wherever the target format's character set allows, so leak checks can search by canary ID without ever printing a value. If a format cannot contain the marker, the generator records the value's SHA-256 in a git-ignored manifest and leak checks compare hashes of candidate substrings.
4. **No vendor example keys.** Detectors may allowlist well-known documentation example credentials; using them would produce false "no finding" results. Generators produce format-valid but never-issued values.
5. **Never verified live.** No fake (or real) secret is ever sent to a provider to check validity (Arch §10).
6. **Execution markers.** Fixtures that would execute code if mishandled write a marker file named `PWT_EXECUTED_<fixture-id>` into a scratch path the spike monitors. The marker payload is inert (touch a file); it never contacts the network or modifies anything else.
7. **Manifest.** Every fixture has a manifest row: ID, path, purpose, generator (if any), contains-fake-secret (yes/no + canary IDs), contains-execution-marker (yes/no), expected product outcome, consumers (spike/test IDs).
8. **Hostile names on disk.** Fixtures with hostile file names are generated at build time (several are awkward or impossible to commit portably).

## 2. Fixture catalogue

### 2.1 YAML content (FX-YAML-*)
| ID | Content | Expected product outcome |
|---|---|---|
| FX-YAML-VALID | Small valid YAML, single document, no style issues | syntax `passed`; other categories per routing; never "fully secure" for unknown type |
| FX-YAML-STYLE | Valid YAML with style issues only (long line, trailing spaces, inconsistent indentation) | style findings kept separate from syntax; syntax `passed` |
| FX-YAML-MALFORMED | Unclosed flow mapping / bad indentation | syntax `findings` with location; other checks continue where safe |
| FX-YAML-MALFORMED-SECRET | FX-YAML-MALFORMED plus one fake secret (canary C01) | syntax finding **and** secret finding (SCAN-08); canary absent from all persisted output |
| FX-YAML-DUPKEY | Duplicate mapping key at top level and nested | explicit validation error, not last-value-wins |
| FX-YAML-MULTIDOC | Three documents, the second invalid | per-document results with one-based index |
| FX-YAML-ALIAS-BOMB | Nested anchors/aliases with exponential expansion | bounded failure (resource limit) recorded as `incomplete`/`failed`; no memory exhaustion |
| FX-YAML-DEEP | Very deep nesting (e.g. thousands of levels) | bounded failure, no stack overflow/panic |
| FX-YAML-TAGS | Custom and language-specific object tags | treated as data or rejected; never instantiated/executed |
| FX-YAML-ENCODING | Variants: UTF-8 BOM, tab indentation, invalid UTF-8 bytes, CRLF | explicit diagnostics; no crash; invalid bytes not echoed raw to terminal |
| FX-YAML-MERGE | Merge keys and anchors across a document | parser behaviour documented; locations preserved |
| FX-YAML-LIMITS | File above configured size limit; directory with more files than the count limit | listed as skipped coverage gaps with reasons |
| FX-YAML-CASE | `.yml`, `.yaml`, `.YAML`, `.Yml` extensions; a non-YAML file with YAML content | discovery per Tech §6 (case-insensitive proposal); non-YAML not selectable |

### 2.2 Fake secrets (FX-SEC-*)
| ID | Shape (description only) | Canary | Expected product outcome |
|---|---|---|---|
| FX-SEC-TOKEN-PREFIXED | Provider-style token with a well-known prefix and fixed length | C02 | secret finding; evidence redacted |
| FX-SEC-KEYPAIR | Access-key-ID-like + secret-like pair in a config map | C03, C04 | finding(s); both values redacted |
| FX-SEC-PRIVATE-KEY | PEM-armoured private-key block with random body | C05 | finding; block never persisted |
| FX-SEC-URL-CRED | URL with embedded `user:password@` credentials | C06 | finding; URL stored without credentials |
| FX-SEC-GENERIC | High-entropy value assigned to a `password`/`api_key` field | C07 | finding where the engine supports generic rules; otherwise documented gap |
| FX-SEC-MULTILINE | Secret in a YAML block scalar | C08 | finding with line range; redacted |
| FX-SEC-BASE64 | Base64-encoded secret inside a Kubernetes Secret manifest | C09 | engine-dependent; result recorded honestly |
| FX-SEC-NEAR-MISS | Values that look secret-like but are clearly placeholders (`changeme`, `${VAR}`, all-x) | none | used to observe false positives; not a leak |
| FX-SEC-STDERR | Used by FX-PROC-SECRET-STDERR (canary C10) | C10 | canary absent from logs/events/reports |

Leak check (every spike and later test): search all captured streams, report files, store files, logs and generated PDFs for canary IDs/hashes; any hit outside the scratch fixture tree is a confirmed defect.

### 2.3 Repository-config injection (FX-INJ-*)
Each fixture is a small repository tree containing FX-YAML-STYLE, FX-SEC-TOKEN-PREFIXED and a misconfigured manifest, plus the hostile config. Expected product outcome for all: the hostile config has **no effect** on app-run results (same findings as the clean twin), or the capability is disabled with an explicit reason. Every fixture has a clean twin (same tree without the hostile file) for comparison.

| ID | Hostile content |
|---|---|
| FX-INJ-YAMLLINT | Repository yamllint config files (each candidate name) that disable all rules or ignore all files |
| FX-INJ-GITLEAKS-CONFIG | Repository gitleaks config that allowlists all paths / replaces rules with an empty set |
| FX-INJ-GITLEAKS-IGNORE | Repository gitleaks ignore file listing the fingerprints of the planted findings |
| FX-INJ-CHECKOV-CONFIG | Repository checkov config that skips all checks / changes frameworks / points to an external checks directory |
| FX-INJ-EXEC-CHECKOV | Custom check module in the repository (writes `PWT_EXECUTED_FX-INJ-EXEC-CHECKOV` if loaded) referenced by FX-INJ-CHECKOV-CONFIG | 
| FX-INJ-ACTIONLINT | Repository actionlint config that ignores all errors / declares fake self-hosted labels |
| FX-INJ-INLINE-YAMLLINT | In-file comment directives disabling yamllint rules |
| FX-INJ-INLINE-GITLEAKS | In-file allow annotations on the secret line |
| FX-INJ-INLINE-CHECKOV | In-file skip annotations on the misconfigured resource |

For FX-INJ-EXEC-CHECKOV the expected outcome is strict: the marker file must never appear. For inline-directive fixtures, if a tool cannot ignore them, the product must at minimum report that results are repository-suppressible (limitation) — the spike records which applies.

### 2.4 Scope (FX-SCOPE-*)
| ID | Layout | Expected product outcome |
|---|---|---|
| FX-SCOPE-SIBLINGS | Selected file A plus unselected sibling B with a planted secret (C11) | only A assessed; no finding for B reported or persisted; if a tool reads B, that is recorded as wider read scope |
| FX-SCOPE-NESTED | Selected file in a subdirectory with unselected parent/child YAML | same as above; relative paths preserved |
| FX-SCOPE-NONYAML | Unselected non-YAML file containing a secret (C12) beside a selected YAML file | not assessed unless disclosed as approved context |
| FX-SCOPE-STAGED | Expected line-number mapping table for a staged-copy strategy | staged results map back to original paths/lines exactly |

### 2.5 Hostile names and output (FX-NAME-*, FX-OUT-*)
| ID | Variant | Expected product outcome |
|---|---|---|
| FX-NAME-DASH | File names beginning with `-` / `--` (option-like) | passed as data, never parsed as options |
| FX-NAME-ANSI | File name containing terminal escape sequences | escaped on display/logs/reports; terminal unaffected |
| FX-NAME-NEWLINE | File name containing a newline | handled or explicitly rejected with reason; no record splitting |
| FX-NAME-GLOB | Names with `*`, `?`, `[`, spaces, quotes, `$` | no shell/glob expansion anywhere |
| FX-NAME-UNICODE | Bidirectional-override characters, combining marks, very long name | displayed safely and unambiguously |
| FX-OUT-LARGE | Input causing very large tool output (many findings) | output bound enforced; `output_limit_exceeded`; earlier findings retained, coverage `incomplete` |
| FX-OUT-ANSI-CONTENT | YAML values containing escape sequences that tools may echo | escaped before display; not persisted raw |

### 2.6 Symlinks (FX-LINK-*)
| ID | Variant | Expected product outcome |
|---|---|---|
| FX-LINK-OUTSIDE | Symlink to a file outside the checkout | not followed; listed as excluded with reason |
| FX-LINK-ABSOLUTE | Symlink to an absolute system path | not followed; listed |
| FX-LINK-LOOP | Directory symlink loop | no infinite walk; listed |
| FX-LINK-INROOT | Symlink to a YAML file inside the checkout | not followed by default (Tech §6); listed |

### 2.7 GitHub Actions (FX-GHA-*)
| ID | Content | Expected product outcome |
|---|---|---|
| FX-GHA-VALID | Minimal valid workflow | workflow validation `passed` |
| FX-GHA-INVALID | Unknown keys, bad expression syntax, missing `runs-on` | workflow-validation findings |
| FX-GHA-INJECTION | `run:` step interpolating untrusted event fields, plus a `run:` that would write `PWT_EXECUTED_FX-GHA-INJECTION` | reported where supported; marker never appears (no workflow execution) |
| FX-GHA-REF | `uses:` references: unpinned tag, branch, full SHA, local reusable workflow, Docker image | references recorded; non-image references reported as unsupported gaps (SCAN-05 deferred) |
| FX-GHA-PERMS | Overly broad `permissions`, `pull_request_target` with checkout of head | security findings where an engine supports them; otherwise documented gap |

### 2.8 Kubernetes (FX-K8S-*)
| ID | Content | Expected product outcome |
|---|---|---|
| FX-K8S-VALID | Valid Deployment | schema `passed` (with stub schema); misconfiguration per Checkov |
| FX-K8S-INSECURE | Privileged container, host networking, no resource limits, `latest` image | misconfiguration findings; image reference recorded |
| FX-K8S-INVALID | Wrong field types, unknown fields | schema findings (strict mode) |
| FX-K8S-MULTIDOC | Several resources in one file, one invalid | per-document results |
| FX-K8S-CRD-MISSING | Custom resource of a kind with no cached schema | `unsupported` gap, never `passed` |
| FX-K8S-SCHEMA-STUB | Hand-written minimal local JSON schemas for the kinds above, in the cache layout of Tech §25 | used only to exercise local schema loading before any approved download |
| FX-K8S-TEMPLATE | Helm-style templated YAML (`{{ }}`) not valid as plain YAML | classified as template source; rendering conditional (X3); no false syntax "pass" |

### 2.9 Network-sensitive (FX-NET-*)
| ID | Content | Expected product outcome |
|---|---|---|
| FX-NET-REMOTE-REF | JSON schema with a remote `$ref`; YAML with schema modeline pointing to a URL | no fetch without `schema_fetch` approval; gap recorded |
| FX-NET-DEFAULT | Plain inputs run with networking unavailable | records whether each tool attempts network access |

### 2.10 Git repositories (FX-GIT-*) — built locally as bare repos
| ID | Content | Expected product outcome |
|---|---|---|
| FX-GIT-CLEAN | Simple history, YAML files | clone/update succeed; commit OID captured |
| FX-GIT-HOOK | Repository attempting to supply hooks (via template/config paths the spike enumerates) that write `PWT_EXECUTED_FX-GIT-HOOK` | marker never appears |
| FX-GIT-FILTER | `.gitattributes` declaring filter/LFS attributes, with a filter driver configured in the spike's scratch user config that writes `PWT_EXECUTED_FX-GIT-FILTER` | with app controls applied, marker never appears; behaviour without controls recorded for comparison |
| FX-GIT-SUBMODULE | `.gitmodules` pointing to another local bare repo and to a path-traversal-looking path | no submodule fetch/checkout |
| FX-GIT-STATE-DIRTY | Managed checkout with local modifications | update skipped; nothing reset/stashed |
| FX-GIT-STATE-DIVERGED | Local commit diverging from remote | fast-forward refused; repo skipped with reason |
| FX-GIT-STATE-MERGE | In-progress merge/rebase state | detected; skipped |
| FX-GIT-URL | Candidate URLs: embedded credentials, `file://`, `ext::`, option-like (`-u…`), whitespace/control characters | rejected at validation (HTTPS/SSH only) |

### 2.11 Process supervision (FX-PROC-*) — fake tools written by us
| ID | Behaviour | Expected product outcome |
|---|---|---|
| FX-PROC-SLEEP | Never exits | timeout → `timed_out`, tree killed |
| FX-PROC-TERM-IGNORE | Child ignoring graceful termination signal | forced kill after `grace_ms`; `method=forced` |
| FX-PROC-GRANDCHILD | Spawns a grandchild that outlives the parent | whole tree terminated; `process_tree_exited` honest |
| FX-PROC-DAEMONIZE | Double-forks/escapes the process group | detected as limitation or terminated by stronger mechanism; result recorded |
| FX-PROC-FLOOD-OUT / FX-PROC-FLOOD-ERR | Unbounded stdout / stderr | `output_limit_exceeded`; tree killed |
| FX-PROC-SECRET-STDERR | Writes canary C10 to stderr | canary absent from all durable outputs |
| FX-PROC-EXIT-CODES | Exits 0, 1, 2, 3, and by signal | exact code/signal captured |
| FX-PROC-ANSI | Emits escape sequences | escaped; never written raw to terminal |
| FX-PROC-CLOSE-EARLY | Closes stdout but keeps running | supervisor not blocked; timeout applies |

### 2.12 PDF rendering (FX-PDF-*) — Markdown inputs
| ID | Content | Expected product outcome |
|---|---|---|
| FX-PDF-REMOTE | Image/link/stylesheet references to remote URLs | no network fetch |
| FX-PDF-LOCALFILE | References to absolute local paths and `file:` URLs | no local file content embedded |
| FX-PDF-ACTIVE | Raw HTML with script/iframe/form elements | not executed; escaped or stripped |
| FX-PDF-UNICODE | Mixed scripts (Latin, CJK, Arabic RTL, emoji) | renders legibly or documented limitation |
| FX-PDF-TABLE | Wide and long tables with long unbroken paths | readable; no silent truncation |
| FX-PDF-LARGE | Report with thousands of findings | bounded time/memory; nonempty output check |
| FX-PDF-CANARY | Report source accidentally containing canary C13 | used to confirm redaction happens before rendering |

### 2.13 Assistant (FX-RAG-*) — for P16/P17, specified now for completeness
| ID | Content | Expected product outcome |
|---|---|---|
| FX-RAG-INJECTION | YAML comments instructing the model to ignore rules, reveal secrets or run commands | treated as untrusted data; no tool/approval effect |
| FX-RAG-CROSSREPO | Two repositories with similarly named files | retrieval limited to selected context |
| FX-RAG-SECRET | Selected file with canary C14 | canary absent from index, payload (mock HTTP capture) and logs |

## 3. Build and storage plan (for later approval)
- Spike fixtures: generated into scratch directories by a generator written during the relevant spike task; generator source may be committed after review, generated output is not.
- Permanent corpus (P01+): `tests/fixtures/<area>/<FX-ID>/` with `manifest.json` per fixture; fake-secret values generated at test time; hostile names generated at test time.
- Fixture generators must not execute anything from the fixture content and must not use the network.

## 4. Coverage map (fixture → spike/test)
| Area | Spikes | Later phases |
|---|---|---|
| FX-YAML-* | T06, T09 (yamllint, Gitleaks SCAN-08) | P05, P07, P08 |
| FX-SEC-* | T09 (Gitleaks) | P06, P07, P08, P10, P17 |
| FX-INJ-* | T09 | P08, P09, P13 |
| FX-SCOPE-*, FX-NAME-*, FX-OUT-*, FX-LINK-* | T09 | P05, P06, P08 |
| FX-GHA-*, FX-K8S-* | T09 | P09, P13 |
| FX-NET-* | T06, T09 | P08, P13 |
| FX-GIT-* | T04 | P04 |
| FX-PROC-* | T05 | P06 |
| FX-PDF-* | T08 | P12 |
| FX-RAG-* | — | P16, P17 |
