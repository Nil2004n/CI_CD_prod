# P00-T10 read-only lookups (2026-10-24, Opus)

**Approval:** owner, 2026-10-24 ("I accept and approve t10"); this resolves Q-06/B-03 for T10.

**Method:** WebFetch of public pages and GitHub REST metadata, read through a summarising fetch tool.
- Not downloaded: schema sets, archives or individual schema files. The one exception is the 2-line `LICENSE` notice of yannh/kubernetes-json-schema.
- Directory listings were checked for existence only.
- Values below are as reported by the fetch tool and may lag the live pages.

| # | URL | Fact recorded |
|---|---|---|
| L1 | https://github.com/yannh/kubernetes-json-schema | Fork of instrumenta/kubernetes-json-schema. "JSON Schemas for every version of every object in every version of Kubernetes". Generated with a fork of openapi2jsonschema (`build.sh`). "kept up-to-date automatically every day using Github Actions". Directory variants `vX.Y.Z`, `-local`, `-standalone`, `-standalone-strict`, and `master*`. |
| L2 | https://api.github.com/repos/yannh/kubernetes-json-schema | `license.spdx_id: NOASSERTION` ("Other"); default branch `master`; `pushed_at 2026-10-23T15:09:06Z`; not archived; fork of instrumenta/kubernetes-json-schema; size 133055 KB. |
| L3 | https://raw.githubusercontent.com/yannh/kubernetes-json-schema/master/LICENSE | Apache License 2.0 notice. "Copyright (C) 2017 Gareth Rushgrove", "Copyright (C) 2017 Yanh Hamon". SPDX detection fails on the notice format, not on the licence. |
| L4 | https://api.github.com/repos/yannh/kubernetes-json-schema/commits?per_page=3 | Latest commits: `f7d855e49c34b039c9ef2196444910275e31e09e` 2026-10-23T15:09:03Z "Adding new schemas"; `b2ba4def…` 2026-10-21; `93153a14…` 2026-10-17. |
| L5 | contents API `v1.37.1-standalone-strict`, `v1.36.5-standalone-strict`, `v1.35.9-standalone-strict` (ref `master`) | All three exist as directory listings (each over 1 MB of listing JSON, truncated by the tool). `_definitions.json` and `all.json` are present in v1.37.1. Specific kind files were not confirmed (listing truncated). |
| L6 | https://github.com/yannh/kubeconform | Apache-2.0. Default location alias expands to `https://raw.githubusercontent.com/yannh/kubernetes-json-schema/master/{{.NormalizedKubernetesVersion}}-standalone{{.StrictSuffix}}/{{.ResourceKind}}{{.KindSuffix}}.json`. Template variables: `NormalizedKubernetesVersion`, `StrictSuffix`, `ResourceKind`, `ResourceAPIVersion`, `Group`, `KindSuffix`. `-cache` = "cache schemas downloaded via HTTP to this folder". Local paths accepted (`.json` value = template). `openapi2jsonschema` (Go, `openapi2jsonschema-go/`) converts CRDs; `FILENAME_FORMAT` options. Datree CRDs-catalog registry template `…/datreeio/CRDs-catalog/main/{{.Group}}/{{.ResourceKind}}_{{.ResourceAPIVersion}}.json`. |
| L7 | https://api.github.com/repos/yannh/kubeconform/releases/latest | `v0.8.0`, published 2026-06-04T20:08:59Z. Matches the installed, hash-verified kubeconform 0.8.0 (P00-T02). |
| L8 | https://kubernetes.io/releases/ | Maintained lines: 1.37 (1.37.1, EOL 2027-10-28), 1.36 (1.36.5, EOL 2027-06-28), 1.35 (1.35.9, EOL 2027-02-28), 1.34 (1.34.12, EOL **2026-10-27**). The page says the project maintains "the most recent three minor releases (1.37, 1.36, 1.35)"; about 1 year of patch support. |
| L9 | https://github.com/kubernetes/kubernetes/tree/master/api/openapi-spec, https://api.github.com/repos/kubernetes/kubernetes | `swagger.json`, a `v3/` directory and a README ("an OpenAPI specification for Kubernetes API"; `x-kubernetes-*` extensions). Repository licence Apache-2.0. |
| L10 | https://github.com/datreeio/CRDs-catalog, https://api.github.com/repos/datreeio/CRDs-catalog | "Popular Kubernetes CRDs … in JSON schema format". Licence MIT. Default branch `main`; `pushed_at 2026-10-18T06:57:38Z`; not archived. Group directories with `kind_version.json` (inferred from README examples). Contribution guide mentions an automatic merge policy (details not visible). |
| L11 | https://github.com/instrumenta/kubernetes-json-schema | Original project. Newest version named in the README is v1.14.0. Points to kubernetesjsonschema.dev. No archive notice. Effectively superseded by the yannh fork. |

## Limitations
- GitHub provides no signatures or attestations for these schema sets. Integrity can only rest on a pinned commit plus self-computed hashes, and Git blob/tree hashes from the API.
- The datreeio contribution model (automatic merges) was not reviewed in detail.
- Per-kind file existence for the target versions was not confirmed (truncated listings).
