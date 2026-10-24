#!/bin/bash
# P00-T09: runs INSIDE `unshare -r -n --pid --fork --mount-proc --kill-child env -i ...`.
# Checks PID 1 and the spike root, sets the address-space limits, then execs the Python orchestrator as
# PID 1 with a two-variable environment.
# RLIMIT_AS (owner decision 2026-10-24): hard 16 GiB so that gitleaks children can be given 16 GiB
# (go-re2/wazero reserves 4 GiB of virtual address space at start-up; 4 GiB killed it); soft 4 GiB for
# the orchestrator itself. common.Ctx.run sets soft=hard per child: gitleaks 16 GiB, all others 4 GiB.
# RLIMIT_AS is a virtual-address-space bound, not a physical-memory cap.
# Python: -I isolated (ignores PYTHON* variables), -B no bytecode writes, pycache_prefix inside the fresh
# spike root so no repository __pycache__ is ever read.
# Usage: setup-and-run.sh <spike-root> <out-dir> <tools> [preflight-only]
set -euo pipefail
root=$1; out=$2; tools=$3; mode=${4:-}
here=$(cd "$(dirname "$0")/.." && pwd)
[ "$$" = 1 ] || { echo "setup: not PID 1" >&2; exit 90; }
case "$root" in /home/kali/.local/share/pwt-dev-tools/_spikes/t09.*) ;; *) echo "setup: bad root" >&2; exit 91;; esac
ulimit -S -v 4194304
ulimit -H -v 16777216
exec env -i PATH=/usr/bin:/bin LANG=C.UTF-8 /usr/bin/python3 -I -B -X pycache_prefix="$root/pycache" \
  "$here/run_t09.py" "$root" "$out" "$tools" $mode
