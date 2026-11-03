# P04-T01 worker assignments

Owner approval 2026-11-03: T01 only; separate branch sessions requested; primary orchestrates, reviews, tests and initiates debug cycles. No commits or pushes until explicitly requested. No network, installs or new dependencies.

All worktrees share base c1832672f12aa41129d882f4d58dc06331df490e. Spawn calls explicitly configured `model: gpt-6.1-terra`, `fork_turns: none`; no independent API-resolved identity is exposed. Worker self-reports are unverified. Native collaboration messages verified by spawn responses. Primary owns adoption of shared-interface drafts and integrated verification.

| Agent | Branch and worktree under /home/debjoyy/Projects | Exclusive files |
|---|---|---|
| /root/t01_request | task/P04-T01-request; P04-T01-request | planning/P04-T01-request.md |
| /root/t01_safety | task/P04-T01-safety; P04-T01-safety | planning/P04-T01-git-safety.md |
| /root/t01_probes | task/P04-T01-probes; P04-T01-probes | evidence/P04/T01/probe_local.py; planning/P04-T01-packets/*.md |

Each spawn packet included task, base revision/branch/worktree, exclusive file ownership, frozen source/schema/manifests, permitted commands, no network or commits, required tests/design matrix, stop condition and report format. Future implementation packets are unapproved templates requiring a verified B0 and owner go. Source implementation T02–T12 is out of scope.
