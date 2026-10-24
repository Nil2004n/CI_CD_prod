#!/bin/bash
# P00-T07: runs INSIDE `unshare -r -n -m --pid --fork --mount-proc --kill-child env -i ...`.
# Masks real bus/keyring/display/service paths with empty tmpfs, then execs the harness as PID 1
# with a two-variable environment. Any failure aborts before a daemon or library call.
# Usage: setup-and-run.sh <artifact> <artifact-sha256> <spike-root> <out-dir> [preflight-only]
set -euo pipefail
artifact=$1; want_sha=$2; root=$3; out=$4; mode=${5:-}

[ "$$" = 1 ] || { echo "setup: not PID 1" >&2; exit 90; }
case "$root" in /home/kali/.local/share/pwt-dev-tools/_spikes/t07.*) ;; *) echo "setup: bad root" >&2; exit 91;; esac

have_sha=$(sha256sum "$artifact" | cut -d' ' -f1)
[ "$have_sha" = "$want_sha" ] || { echo "setup: artifact hash mismatch" >&2; exit 92; }

masks=(/run/user /run/dbus /home/kali/.local/share/keyrings /tmp/.X11-unix /usr/share/dbus-1/services /etc/dbus-1/session.d)
for p in "${masks[@]}"; do
  [ -d "$p" ] || { echo "setup: mask target missing: $p" >&2; exit 93; }
  mount -t tmpfs -o mode=0700,size=1m,nosuid,nodev,noexec tmpfs "$p" || { echo "setup: mount failed: $p" >&2; exit 94; }
done

exec env -i PATH=/usr/bin:/bin LANG=C.UTF-8 "$artifact" orchestrate "$root" "$out" $mode
