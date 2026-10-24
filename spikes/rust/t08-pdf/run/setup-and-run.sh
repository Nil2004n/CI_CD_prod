#!/bin/bash
# P00-T08: runs INSIDE `unshare -r -n --pid --fork --mount-proc --kill-child env -i ...`.
# Checks PID 1, root prefix and artifact hash, applies the 2 GiB address-space limit, then
# execs the harness as PID 1 with a two-variable environment.
# Usage: setup-and-run.sh <artifact> <artifact-sha256> <spike-root> <out-dir> [preflight-only]
set -euo pipefail
artifact=$1; want_sha=$2; root=$3; out=$4; mode=${5:-}

[ "$$" = 1 ] || { echo "setup: not PID 1" >&2; exit 90; }
case "$root" in /home/kali/.local/share/pwt-dev-tools/_spikes/t08.*) ;; *) echo "setup: bad root" >&2; exit 91;; esac
have_sha=$(sha256sum "$artifact" | cut -d' ' -f1)
[ "$have_sha" = "$want_sha" ] || { echo "setup: artifact hash mismatch" >&2; exit 92; }

ulimit -v 2097152
exec env -i PATH=/usr/bin:/bin LANG=C.UTF-8 "$artifact" orchestrate "$root" "$out" $mode
