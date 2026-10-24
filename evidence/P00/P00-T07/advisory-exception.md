# P00-T07 advisory exception (spike-only)

Decision: owner chose option **K-A** on 2026-10-24. This file records the exception exactly; it is not an advisory-ignore configuration and no `audit.toml`/`deny.toml` or global ignore rule exists.

## Covered advisories (and nothing else)

| Advisory | Crate | Version (locked) | Checksum (Cargo.lock) | Kind |
|---|---|---|---|---|
| RUSTSEC-2026-0370 (2026-10-16) | zvariant | 5.15.1 | `80bf6d6cd6a4b601acb64cfb8284cdc643fcfa75ef26ae3c46ec8aa4e8a1fbb7` | informational: unmaintained |
| RUSTSEC-2026-0371 (2026-10-16) | zbus_names | 4.3.5 | `812a12be4ead7a59e18ea8f047d08add0066b73bc62fc1467066a2472ae088dd` | informational: unmaintained |
| RUSTSEC-2026-0372 (2026-10-16) | zvariant_derive | 5.15.1 (proc-macro) | `ef2672456ba1cb54b5eaec136e63135b3c7495fb938e53b28a62e633496d486d` | informational: unmaintained |

Bound to `spikes/rust/Cargo.lock` SHA-256 `eeca61f5f8b505efd2985bebb0466f5d7f6c98ca8fd7ea8624287b3877bf099c` (after A, unchanged by B).

Dependency paths on the host target (from `cargo tree -i`, `B-excepted-paths.txt`):
- `t07-keyring → keyring 4.2.0 → zbus-secret-service-keyring-store 1.0.1 → zbus 5.19.0 → {zvariant 5.15.1, zbus_names 4.3.5}` (also directly from `zbus-secret-service-keyring-store` and `secret-service 5.2.0` via `zbus`)
- `zbus 5.19.0 → zbus_macros 5.19.0 (proc-macro) → {zvariant, zbus_names}` (compile time)
- `zvariant 5.15.1 → zvariant_derive 5.15.1 (proc-macro)` (compile time)

## Classification
The advisories are informational `unmaintained` notices: the crates were consolidated into `zbus` 6.0.0 (`names`/`wire` modules and derive macros; upstream issue z-galaxy/zbus#2043). They are not reports of a known exploitable vulnerability. The A2 screen found no vulnerability or unsoundness advisory affecting the locked versions.

## Reason
The Secret Service behaviour questions behind KEY-01 (absent bus, absent service, locked collection, unresponsive bus, no plaintext fallback) do not depend on how zbus is split into crates. No `secret-service`/`zbus-secret-service-keyring-store` release uses `zbus` 6 (cached index, 2026-10-24), so waiting would block the feasibility answer without any change to the behaviour that is measured.

## Remaining risk
Security fixes are no longer expected for these crates as separate releases. A future fix would land only in `zbus` 6, which the current backend path cannot use. `zvariant` parses D-Bus wire data received from the bus peer (here, only a private spike daemon).

## Allowed scope
- Only P00-T07 stages B (done), and C/D if approved separately, on this host. The scope is limited to the exact versions, checksums and paths above in this lockfile.
- It does **not** extend to another advisory, another version of these crates, a different dependency graph (any re-resolution voids it), P01+ workspaces, or any production or release build.
- If a new applicable vulnerability, unsoundness or other functional/security advisory appears for any crate in this graph, stop and report.

## Mandatory review points
1. **Before P03 / backend adoption:** re-screen the whole dependency path. Check whether a maintained, compatible upstream release exists (e.g. a `zbus` 6-based `secret-service`/keyring store). Explicitly re-decide this exception. Demonstrate the required error/timeout/no-plaintext behaviour (see the request §5).
2. **Before any release:** re-screen again; an unmaintained-crate exception cannot carry into a release without a fresh owner decision.

Persistent OS credential storage is **not accepted** for the pilot or product by this exception. Session-only mode remains required and must not depend on a working desktop/keyring service. K-D (dropping OS-store mode) has not been chosen; the requirement stays on the roadmap pending a later explicit product decision.

## Status at P00 closure (owner decision, 2026-10-24)
**Historical / inactive.**
- It remains valid only for the completed, authorised P00-T07 experiment, with its original lockfile (`eeca61f5…`) and scope as recorded above.
- It does **not** apply to the current frozen lockfile (`a81eec56…`, changed by the lock-preserving T08 resolution).
- No additional keyring build or production adoption is authorised. A future build or adoption requires a new explicit owner decision and an advisory re-screen (FR-24).
- The exception is not extended, its original scope is unchanged, and the advisories are not suppressed globally.
- The frozen dependency inventory (`spikes/p00-freeze/crates.json`) is distinct from permission to build or adopt.
