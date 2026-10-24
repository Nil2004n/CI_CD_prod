#!/bin/bash
# P00-T07: runs OUTSIDE the namespaces. Creates the spike-owned root, records metadata-only
# before/after snapshots of real paths (stat / names+mtime hashed; never opened or connected),
# runs setup-and-run.sh in the namespaces under an outer watchdog, copies sanitised results,
# and removes only the spike-owned root.
# Usage: outer.sh <artifact> <evidence-dir> [preflight-only]
set -uo pipefail
artifact=$1; ev=$2; mode=${3:-}
here=$(cd "$(dirname "$0")" && pwd)
base=/home/kali/.local/share/pwt-dev-tools/_spikes
mkdir -p "$ev"

snap() {
  {
    stat -c 'bus %i %F %Y' /run/user/1000/bus 2>/dev/null || echo "bus absent"
    # names+mtime+size of the real keyring dir, hashed so no names are recorded
    find /home/kali/.local/share/keyrings -maxdepth 1 -printf '%f %T@ %s\n' 2>/dev/null | sort | sha256sum | cut -d' ' -f1
    pgrep -u kali -x dbus-daemon | sort | tr '\n' ' '; echo
    pgrep -u kali -f gnome-keyring-d | sort | tr '\n' ' '; echo
  }
}

sha=$(sha256sum "$artifact" | cut -d' ' -f1)
root=$(mktemp -d "$base/t07.XXXXXX") || exit 80
chmod 0700 "$root"
mkdir -m 0700 "$root/out"
snap > "$ev/outer-before.txt"
start=$(date -u +%Y-%m-%dT%H:%M:%SZ)

timeout -s KILL 240 unshare -r -n -m --pid --fork --mount-proc --kill-child \
  env -i PATH=/usr/bin:/bin LANG=C.UTF-8 \
  bash "$here/setup-and-run.sh" "$artifact" "$sha" "$root" "$root/out" $mode \
  > "$ev/run.stdout" 2> "$ev/run.stderr"
rc=$?

snap > "$ev/outer-after.txt"
{
  echo "start=$start end=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "artifact_sha256=$sha"
  echo "exit=$rc"
  if cmp -s "$ev/outer-before.txt" "$ev/outer-after.txt"; then echo "real_path_snapshot=unchanged"; else echo "real_path_snapshot=CHANGED"; fi
} > "$ev/outer-summary.txt"

# Copy sanitised outputs only: results.json and per-case harness/daemon logs (scanned by K05).
cp "$root/out/results.json" "$ev/results.json" 2>/dev/null
if [ -d "$root/cases" ]; then
  for c in "$root"/cases/*; do
    id=$(basename "$c"); mkdir -p "$ev/cases/$id"
    for f in "$c"/out/*; do [ -f "$f" ] && head -c 65536 "$f" > "$ev/cases/$id/$(basename "$f")"; done
  done
fi

# Remove only the spike-owned root (exact mktemp path, prefix and owner checked).
case "$root" in "$base"/t07.??????)
  if [ "$(stat -c %U "$root")" = kali ]; then rm -rf --one-file-system -- "$root"; fi;;
esac
[ -e "$root" ] && echo "cleanup: root still present" >> "$ev/outer-summary.txt" || echo "cleanup: root removed" >> "$ev/outer-summary.txt"
exit $rc
