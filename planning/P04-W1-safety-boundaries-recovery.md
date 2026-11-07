# P04 wave 1 — recovery design and unresolved boundaries

2026-11-04. This is a design assessment, not implementation or acceptance evidence. The integrated runner, SSH and checkout deletion remain fail-closed.

## Runner

A process group led by GNU timeout can be signalled by a pinned helper only while its numeric identity cannot be reused. Holding an unreaped child can preserve that identity only if no other process-wide reaper or SIGCHLD auto-reaping consumes it; the current application contract does not establish this. Waiting before signalling can release the identity. The helper's failure and its own bounded lifecycle must also be handled. Blocking pipe threads may remain stuck if an escaped descendant retains a pipe; unbounded detached threads or reapers are not an acceptable cleanup guarantee. A dedicated supervisor with exclusive child ownership and a reviewed safe nonblocking I/O and signalling mechanism is a candidate, but its own startup, bounded cancellation and failure cleanup need design and verification. Do not enable the current runner on these assumptions alone.

The environment should be constructed from opaque, revalidated app-owned HOME and XDG directory capabilities and fixed settings, with `env_clear`; an approved SSH socket must be bound to the same validated SSH policy. Global-helper extraction needs its own narrow profile. Tests must cover exact environment and seeded canaries, simultaneous floods, blocked input, timeout and escalation, lingering grandchildren, helper failure, competing reapers and unrelated-group controls, bounded repeated failures, and the documented escaped-session limitation.

## SSH

`TrustedSshPolicy` is not connected to the refusing `FixedSshPolicy`. A network-only typed constructor should derive the fixed command and agent from the same revalidated policy, never from caller-supplied overrides, and require repository configuration screening before execution. Positive policy construction and replacement tests require a trusted private ancestor and synthetic socket; the existing `ssh -G` test does not establish those or passwd-home isolation. Path checks cannot prevent a same-user actor from replacing resources between revalidation and use. Runner supervision is a prerequisite.

## Checkout deletion

Descriptor-relative no-follow traversal with mount confinement, held directory descriptors and a private quarantine is a candidate for cooperating processes. It requires a reviewed safe wrapper or adapter: direct unsafe Rust is forbidden, and the existing lock capability exposes paths rather than held directory descriptors. A malicious same-UID actor can move an already-open child outside the checkout before a descriptor-relative unlink, or replace a name between identity check and unlink. Neither `openat2` nor private mode 0700 alone prevents this. Thus the existing requirement to prove no outside modification in replacement races cannot be claimed from this design against such an actor.

Two possible directions require a scoped owner decision: (1) retain the existing boundary and investigate enforceable isolation from same-UID namespace manipulation, or (2) amend the boundary to exclude malicious concurrent same-UID namespace and mount manipulation, clearly disclose that limitation, and implement the descriptor-relative design with race and potency tests. Both require a dependency and integration review before use. Partial deletion must be reported and reconciled; it is not atomic rollback.

No tests, dependency changes, network access, real credentials or data, commit, push, B1 or security acceptance are represented by this document.
