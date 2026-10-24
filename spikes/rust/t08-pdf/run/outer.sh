#!/bin/bash
# P00-T08: runs OUTSIDE the namespaces. Creates the spike-owned root, records the allowlisted
# font hashes, runs setup-and-run.sh under the 900 s watchdog, copies sanitised text evidence,
# moves verified PDFs to an artifact directory outside source control, and removes only the
# spike-owned root.
# Usage: outer.sh <artifact> <evidence-dir> <artifact-dir> [preflight-only]
set -uo pipefail
artifact=$1; ev=$2; art=$3; mode=${4:-}
here=$(cd "$(dirname "$0")" && pwd)
base=/home/kali/.local/share/pwt-dev-tools/_spikes
fonts=(/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
       /usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf
       /usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf)
mkdir -p "$ev"

sha=$(sha256sum "$artifact" | cut -d' ' -f1)
root=$(mktemp -d "$base/t08.XXXXXX") || exit 80
chmod 0700 "$root"
mkdir -m 0700 "$root/out"
sha256sum "${fonts[@]}" > "$root/fonts.sha256"
cp "$root/fonts.sha256" "$ev/fonts-before.sha256"
start=$(date -u +%Y-%m-%dT%H:%M:%SZ)

timeout -s KILL 900 unshare -r -n --pid --fork --mount-proc --kill-child \
  env -i PATH=/usr/bin:/bin LANG=C.UTF-8 \
  bash "$here/setup-and-run.sh" "$artifact" "$sha" "$root" "$root/out" $mode \
  > "$ev/run.stdout" 2> "$ev/run.stderr"
rc=$?

sha256sum "${fonts[@]}" > "$ev/fonts-after.sha256"
{
  echo "start=$start end=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "artifact_sha256=$sha"
  echo "exit=$rc"
  cmp -s "$ev/fonts-before.sha256" "$ev/fonts-after.sha256" && echo "fonts_unchanged=yes" || echo "fonts_unchanged=NO"
} > "$ev/outer-summary.txt"

cp "$root/out/results.json" "$ev/results.json" 2>/dev/null
if [ -d "$root/cases" ]; then
  for c in "$root"/cases/*; do
    id=$(basename "$c"); mkdir -p "$ev/cases/$id"
    # Sanitised text only: render JSON, tool stdout/stderr, bounded extraction excerpt.
    # C1 (canary) extraction and inputs are never copied.
    for f in render.stdout render.stderr mutool-info.stdout mutool-info.stderr mutool-draw.stderr mutool-clean.stderr; do
      [ -f "$c/$f" ] && head -c 65536 "$c/$f" > "$ev/cases/$id/$f"
    done
    if [ "$id" != C1 ] && [ -f "$c/extracted.txt" ]; then head -c 16384 "$c/extracted.txt" > "$ev/cases/$id/extracted-excerpt.txt"; fi
  done
fi
if [ -d "$root/pdf" ] && ls "$root"/pdf/*.pdf >/dev/null 2>&1; then
  mkdir -p "$art"; mv "$root"/pdf/*.pdf "$art"/; (cd "$art" && sha256sum *.pdf) > "$ev/artifact-pdfs.sha256"
fi

case "$root" in "$base"/t08.??????)
  if [ "$(stat -c %U "$root")" = kali ]; then rm -rf --one-file-system -- "$root"; fi;;
esac
[ -e "$root" ] && echo "cleanup: root still present" >> "$ev/outer-summary.txt" || echo "cleanup: root removed" >> "$ev/outer-summary.txt"
exit $rc
