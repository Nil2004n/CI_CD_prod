# P04 separate checkout authority — design direction

2026-11-04. The owner asked to pursue a separate trusted authority for checkout operations and process ownership. This document is a proposed design, not an established safety guarantee or authorization to provision accounts, run services, migrate data, or use real credentials.

**Owner choice:** use an administrator authentication prompt for the independent approval direction. The mechanism, trusted presentation of the exact operation, decision binding, and resistance to cached or replayed authorization remain to be designed and tested. The presence of `pkexec` does not prove these properties. This choice does not itself authorize account provisioning, installing policy, or use of real credentials.

## Authority and checkout ownership

A dedicated non-login service UID, established by an administrator, exclusively owns the checkout root and every managed checkout from creation. The ordinary client UID and unrelated processes must be unable to mutate directory entries, acquire writable file descriptors, alter ACLs or mounts, or execute arbitrary code under the authority UID. Admin-controlled executable, configuration and socket paths must be verified at startup. A separate UID alone is insufficient; startup must fail closed when ownership, permissions, ACLs, mount topology or executable identity cannot be established. Privileged administrators, the authority itself and kernel compromise are outside this boundary.

The authority performs every clone, fetch, checkout, rename and deletion, not just deletion. A versioned local socket accepts bounded typed operations with repository IDs and validated targets, never arbitrary paths, commands or inherited file descriptors. Kernel peer credentials and authorization restrict callers, but same-UID socket authentication alone cannot distinguish an authorized client from an adversarial process of that UID. The authority must independently validate operation authorization and hold the per-repository lock through mutation and final state handling. It uses held directory descriptors, no-follow traversal, mount confinement, ownership and identity checks. Failure, including partial deletion, is reported and reconciled; rollback is not assumed.

## Credential and reading boundary

The approved E3 policy may execute user-controlled credential helpers. They must not run with authority privileges or obtain writable checkout descriptors. A separately reviewed, bounded credential broker running under the client identity is a candidate; SSH agent and known-host access require equivalent explicit mediation. Secret lifetime, authorization, redaction, protocol, denial and crash behavior remain unspecified. Until proven, affected network operations refuse. Git itself remains pinned and hardened; hooks, filters, repository configuration and arbitrary programs cannot execute as the authority.

Candidate broker protocol: authority Git invokes a fixed app-owned adapter, which exchanges bounded, framed credential get/store/erase requests over a private per-operation channel with a broker running under the ordinary client UID. The broker performs the approved direct-global-file helper extraction, including ordering and empty resets, and invokes user-selected helpers only as that UID from a private non-repository directory without checkout descriptors. Responses stay on the private channel, are size- and time-limited and are never logged. The broker must match Git credential semantics, including protocol/host/path, `useHttpPath`, helper order, quit and store/erase; use synthetic parity tests against pinned Git rather than assume a custom parser matches it.

An authority-issued one-use capability should bind each channel to the operation ID and approved origin; enforce origin and redirect rules and reject replay, malformed or oversized messages, prompt requests and unauthorized operations. Peer UID alone is insufficient against another process of that UID. Capability issuance, trusted delivery without leakage to helpers, and independent approval verification are unresolved. Broker failure or timeout must abort the operation and trigger partial-state reconciliation. SSH agent and known-host access require separate host-bound mediation; simply forwarding an agent socket is not an established safe design.

Verification must exercise helper order/reset and get/store/erase parity, malformed framing, floods, timeout and quit behavior, secret canaries, replay and cross-origin requests, helper attempts to access the authority checkout, broker crashes and distinct-UID potency controls. This is a proposal; no broker or authorization guarantee is implemented.

Scanners and clients need a separately reviewed read-only snapshot or export, with no hardlinks back to the protected tree and explicit limits and symlink semantics. This changes direct checkout access and dirty-worktree behavior, so compatibility must be resolved before claiming P04 functionality.

## Recovery and supervision

Process ownership and bounded I/O, timeout, group signalling, reaping and failure cleanup remain separate requirements. A service UID does not solve competing reapers, ownership loss or escaped sessions. Durable operation intent and recovery must reconcile checkout state with registry and event publication after crashes or partial failures. Existing same-user writable checkouts must not be silently chowned or deleted; a separately authorized, reviewed migration or re-clone is required.

## Verification and open design work

Before enabling deletion, test with genuinely distinct host UIDs and a positive control that demonstrates attacks against an unprotected fixture. Exercise concurrent rename, replacement, symlink, hardlink and mount attempts; verify outside content remains unchanged; test unauthorized socket requests, helper isolation, held descriptors, crash and partial-publication recovery. Test child and grandchild cleanup, loss of process ownership and unrelated-group controls independently. Host provisioning, privileged setup, real credentials and real remote checks need their applicable explicit authorization. No such provisioning or tests have been performed for this proposal.

The exact authority launch mechanism, peer authorization, credential broker, scanner access, process supervisor and compatibility/migration contract remain to be designed and validated. Production execution and deletion remain fail-closed.
