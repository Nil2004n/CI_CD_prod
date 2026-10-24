#!/bin/bash
# P00-T09: runs OUTSIDE the namespaces. Creates the spike-owned root, runs the orchestrator under a
# watchdog in user+net+PID namespaces, copies only the sanitised results.json, removes the root.
# Usage: outer.sh <evidence-dir> <tools comma-separated> [preflight-only]
set -uo pipefail
ev=$1; tools=$2; mode=${3:-}
here=$(cd "$(dirname "$0")" && pwd)
base=/home/kali/.local/share/pwt-dev-tools/_spikes
mkdir -p "$ev" "$base"
root=$(mktemp -d "$base/t09.XXXXXX") || exit 80
chmod 0700 "$root"; mkdir -m 0700 "$root/out"
start=$(date -u +%Y-%m-%dT%H:%M:%SZ)
# Host namespace identities, for the in-namespace preflight to prove separation.
{ echo "net=$(readlink /proc/self/ns/net)"; echo "user=$(readlink /proc/self/ns/user)"; echo "pid=$(readlink /proc/self/ns/pid)"; } > "$root/host-ns.txt"
timeout -s KILL 1800 unshare -r -n --pid --fork --mount-proc --kill-child \
  env -i PATH=/usr/bin:/bin LANG=C.UTF-8 \
  bash "$here/setup-and-run.sh" "$root" "$root/out" "$tools" $mode \
  > "$ev/run.stdout" 2> "$ev/run.stderr"
rc=$?
{ echo "start=$start end=$(date -u +%Y-%m-%dT%H:%M:%SZ)"; echo "tools=$tools"; echo "exit=$rc"; } > "$ev/outer-summary.txt"
cp "$root/out/results.json" "$ev/results.json" 2>/dev/null
case "$root" in "$base"/t09.??????)
  if [ "$(stat -c %U "$root")" = kali ]; then rm -rf --one-file-system -- "$root"; fi;;
esac
[ -e "$root" ] && echo "cleanup: root still present" >> "$ev/outer-summary.txt" || echo "cleanup: root removed" >> "$ev/outer-summary.txt"
exit $rc
