#!/bin/bash
# P00-T04 spike: local Git-safety checks for clone/update (spike code, not application code).
# Must run as PID 1 of a dedicated user+net+PID namespace (see evidence/P00/P00-T04/record.md).
# Usage: run_t04.sh <scratch-dir (must not exist)> <results.tsv>
# Only synthetic local repositories and synthetic canary helpers created below are used.
set -u
[ "$$" -eq 1 ] || { echo "refusing to run: pid $$ is not 1 (dedicated PID namespace required)"; exit 3; }
[ $# -eq 2 ] || { echo "usage: $0 <scratch-dir> <results.tsv>"; exit 2; }
SCR=$1; RESULTS=$2
[ -e "$SCR" ] && { echo "refusing: scratch $SCR already exists"; exit 2; }
mkdir -p "$SCR"/{logs,canary,homes} "$SCR/empty-template" || exit 2
: > "$RESULTS"
GIT=/usr/bin/git
N=0

# --- Git invocations -------------------------------------------------------------------------
# Every call uses a stripped environment, a scratch HOME (so only scratch global config is read),
# no system config, no terminal prompts, and a 30 s timeout. stdout/stderr go to per-call logs.
base_env() {
  env -i PATH=/usr/bin:/bin LANG=C HOME="$H" GIT_CONFIG_NOSYSTEM=1 GIT_TERMINAL_PROMPT=0 \
    GIT_AUTHOR_NAME=fx GIT_AUTHOR_EMAIL=fx@example.invalid \
    GIT_COMMITTER_NAME=fx GIT_COMMITTER_EMAIL=fx@example.invalid "$@"
}
HARD=(-c core.hooksPath=/dev/null -c core.fsmonitor=false -c protocol.allow=never
      -c protocol.https.allow=always -c protocol.ssh.allow=always -c submodule.recurse=false)
TESTFILE=(-c protocol.file.allow=always)   # test-only, command-scoped local transport exception
logf() { N=$((N+1)); LAST_LOG="$SCR/logs/$(printf %03d $N).log"; }
g_plain()     { logf; base_env timeout 30 "$GIT" "$@" > "$LAST_LOG" 2>&1; }
g_hard()      { logf; base_env timeout 30 "$GIT" "${HARD[@]}" "$@" > "$LAST_LOG" 2>&1; }
g_hard_file() { logf; base_env timeout 30 "$GIT" "${HARD[@]}" "${TESTFILE[@]}" "$@" > "$LAST_LOG" 2>&1; }
g_inherit()   { logf; timeout 30 "$GIT" "$@" > "$LAST_LOG" 2>&1; }   # deliberately inherits caller env
g_hard_data() { local out=$1; shift; logf; base_env timeout 30 "$GIT" "${HARD[@]}" "$@" > "$out" 2> "$LAST_LOG"; }
lastline() { tail -n 1 "$LAST_LOG" 2>/dev/null | sed "s#$SCR#<SCR>#g" | tr '\t' ' ' | cut -c1-160; }

# --- Candidate hardened procedures (D01: fail closed, every step checked explicitly) -----------
EXPECTED_ATTR='* -filter'
REASON=
attr_content_ok() {  # dest: attributes override present with exactly the expected content
  local c
  [ -f "$1/.git/info/attributes" ] || return 1
  IFS= read -r -d '' c < "$1/.git/info/attributes"
  [ "$c" = "$EXPECTED_ATTR"$'\n' ]
}
attr_setup() {  # dest
  local d=$1
  if ! mkdir -p -- "$d/.git/info" || [ ! -d "$d/.git/info" ]; then REASON=ATTR_DIR_FAILED; return 1; fi
  if ! printf '%s\n' "$EXPECTED_ATTR" > "$d/.git/info/attributes"; then REASON=ATTR_WRITE_FAILED; return 1; fi
  attr_content_ok "$d" || { REASON=ATTR_CONTENT_MISMATCH; return 1; }
}
attr_verify() {  # dest target-commit: every path of the target tree must have filter=unset
  local d=$1 t=$2 i n
  local -a P A
  g_hard_data "$SCR/paths.z" -C "$d" ls-tree -r -z --name-only "$t" || { REASON=ATTR_LSTREE_FAILED; return 1; }
  mapfile -d '' -t P < "$SCR/paths.z"
  n=${#P[@]}
  [ "$n" -ge 1 ] || { REASON=ATTR_NO_PATHS; return 1; }
  g_hard_data "$SCR/attrs.z" -C "$d" check-attr -z --source="$t" --stdin filter < "$SCR/paths.z" || { REASON=ATTR_CHECK_FAILED; return 1; }
  mapfile -d '' -t A < "$SCR/attrs.z"
  [ "${#A[@]}" -eq $((3 * n)) ] || { REASON="ATTR_MALFORMED(${#A[@]}/$((3 * n)))"; return 1; }
  for ((i = 0; i < n; i++)); do
    if [ "${A[3*i]}" != "${P[i]}" ] || [ "${A[3*i+1]}" != filter ] || [ "${A[3*i+2]}" != unset ]; then
      REASON="ATTR_NOT_UNSET(${A[3*i+2]})"; return 1
    fi
  done
  VERIFIED_PATHS=$n
}
target_of() { base_env timeout 30 "$GIT" "${HARD[@]}" -C "$1" rev-parse --verify -q "$2^{commit}"; }
hard_clone() {  # url dest [fault] -> 0 cloned+checked out; 1 aborted before checkout (REASON set)
  local u=$1 d=$2 fault=${3:-} t
  REASON=
  g_hard_file clone --no-checkout --template="$SCR/empty-template" "$u" "$d" || { REASON=CLONE_FAILED; return 1; }
  t=$(target_of "$d" HEAD) && [ -n "$t" ] || { REASON=NO_TARGET; return 1; }
  [ "$fault" = attr_dir_blocked ] && : > "$d/.git/info"          # fixture-owned fault: file where the dir must be
  attr_setup "$d" || return 1
  [ "$fault" = weak_override ] && printf '*.txt -filter\n' > "$d/.git/info/attributes"   # fixture-owned fault
  attr_verify "$d" "$t" || return 1
  g_hard -C "$d" checkout -f || { REASON=CHECKOUT_FAILED; return 1; }
  [ "$(target_of "$d" HEAD)" = "$t" ] || { REASON=HEAD_MISMATCH; return 1; }
}
hard_update() {  # dest [fault] -> prints UPDATED <oid> | REFUSED_* | FETCH_FAILED
  local d=$1 fault=${2:-} f t
  REASON=
  for f in MERGE_HEAD CHERRY_PICK_HEAD REVERT_HEAD rebase-merge rebase-apply; do
    [ -e "$d/.git/$f" ] && { echo REFUSED_INPROGRESS; return; }
  done
  g_hard -C "$d" status --porcelain=v1 --untracked-files=all || { echo REFUSED_STATUS_FAILED; return; }
  [ -s "$LAST_LOG" ] && { echo REFUSED_DIRTY; return; }
  g_hard_file -C "$d" fetch origin || { echo FETCH_FAILED; return; }
  t=$(target_of "$d" refs/remotes/origin/main) && [ -n "$t" ] || { echo REFUSED_NO_TARGET; return; }
  [ "$fault" = attr_removed ] && rm -f -- "$d/.git/info/attributes"                         # fixture-owned fault
  [ "$fault" = weak_override ] && printf '*.txt -filter\n' > "$d/.git/info/attributes"   # fixture-owned fault
  if [ "$fault" != weak_override ]; then attr_content_ok "$d" || { echo REFUSED_ATTR_SETUP; return; }; fi
  attr_verify "$d" "$t" || { echo "REFUSED_ATTR_VERIFY:$REASON"; return; }
  g_hard -C "$d" merge --ff-only "$t" || { echo REFUSED_DIVERGED; return; }
  [ "$(target_of "$d" HEAD)" = "$t" ] || { echo REFUSED_HEAD_MISMATCH; return; }
  echo "UPDATED $t"
}
need_clone() {  # url dest case: hardened clone that a case depends on; records a failing SETUP row otherwise
  hard_clone "$1" "$2" && return 0
  rec "$3" setup cloned "clone_aborted:$REASON" fail "$(lastline)"; return 1
}

# --- Fixture helpers ---------------------------------------------------------------------------
home() { H="$SCR/homes/$1"; mkdir -p "$H"; }
canary_dir() { CD="$SCR/canary/$1"; mkdir -p "$CD"; }
fired() { find "$CD" -type f -name 'fired-*' | wc -l; }
clear_canary() { find "$CD" -type f -name 'fired-*' -delete; }
mk_helper() {  # path mode(ok|cat|fail): known-benign synthetic canary; only touches a marker file
  cat > "$1" <<EOF
#!/bin/sh
: > "$CD/fired-\$(basename "\$0")-\$\$"
EOF
  case $2 in cat) echo 'exec cat' >> "$1";; fail) echo 'exit 1' >> "$1";; ok) echo 'exit 0' >> "$1";; esac
  chmod 755 "$1"
}
mk_src() {  # name: work repo on branch main with one commit
  local s="$SCR/$1"; mkdir -p "$s"
  g_plain init -q -b main "$s" && printf 'line1\n' > "$s/a.txt" && g_plain -C "$s" add a.txt && g_plain -C "$s" commit -q -m init
}
commit_file() { printf '%s\n' "$3" > "$SCR/$1/$2"; g_plain -C "$SCR/$1" add "$2"; g_plain -C "$SCR/$1" commit -q -m "change $2"; }
oid() { base_env "$GIT" -C "$1" rev-parse HEAD; }
snap() {  # HEAD + worktree content digest (excluding .git)
  (cd "$1" && { base_env "$GIT" rev-parse HEAD; find . -path ./.git -prune -o -type f -print0 | sort -z | xargs -0 -r sha256sum; } | sha256sum | cut -c1-16)
}
rec() {  # case role expected observed status detail
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" "$5" "$(printf '%s' "$6" | sed "s#$SCR#<SCR>#g" | tr '\t\n' '  ' | cut -c1-240)" >> "$RESULTS"
}
verdict() { [ "$1" = "$2" ] && echo pass || echo fail; }

echo "git: $("$GIT" --version)"; echo "scratch: <SCR>"
H0=; home clean; HC=$H

# G01 initial clone ----------------------------------------------------------------------------
H=$HC; mk_src s1
if hard_clone "file://$SCR/s1" "$SCR/m1"; then
  o=$( [ "$(oid "$SCR/m1")" = "$(oid "$SCR/s1")" ] && [ -f "$SCR/m1/a.txt" ] && echo cloned_same_oid || echo mismatch)
else o=clone_failed; fi
rec G01 behaviour cloned_same_oid "$o" "$(verdict cloned_same_oid "$o")" "oid=$(oid "$SCR/m1" 2>/dev/null | cut -c1-12)"

# G02 safe fast-forward update -----------------------------------------------------------------
commit_file s1 a.txt line2
r=$(hard_update "$SCR/m1"); o=${r%% *}
[ "$o" = UPDATED ] && [ "${r#UPDATED }" = "$(oid "$SCR/s1")" ] && o=updated_to_upstream
rec G02 behaviour updated_to_upstream "$o" "$(verdict updated_to_upstream "$o")" "$r"

# G03a dirty tracked file ----------------------------------------------------------------------
need_clone "file://$SCR/s1" "$SCR/m3a" G03a; printf 'local edit\n' >> "$SCR/m3a/a.txt"; commit_file s1 a.txt line3
b=$(snap "$SCR/m3a"); r=$(hard_update "$SCR/m3a"); a=$(snap "$SCR/m3a")
o="$r"; [ "$b" = "$a" ] && o="$r+unchanged"
rec G03a behaviour REFUSED_DIRTY+unchanged "$o" "$(verdict REFUSED_DIRTY+unchanged "$o")" "snap $b -> $a"

# G03b untracked file that the update would overwrite -------------------------------------------
need_clone "file://$SCR/s1" "$SCR/m3b" G03b; commit_file s1 b.txt upstream-b; printf 'local untracked\n' > "$SCR/m3b/b.txt"
b=$(snap "$SCR/m3b"); r=$(hard_update "$SCR/m3b"); a=$(snap "$SCR/m3b")
o="$r"; [ "$b" = "$a" ] && o="$r+unchanged"
rec G03b behaviour REFUSED_DIRTY+unchanged "$o" "$(verdict REFUSED_DIRTY+unchanged "$o")" "snap $b -> $a"

# G03c in-progress conflicting merge -----------------------------------------------------------
need_clone "file://$SCR/s1" "$SCR/m3c" G03c
printf 'local conflicting\n' > "$SCR/m3c/a.txt"; g_hard -C "$SCR/m3c" commit -q -am local
commit_file s1 a.txt upstream-conflicting
g_hard_file -C "$SCR/m3c" fetch origin; g_hard -C "$SCR/m3c" merge refs/remotes/origin/main
setup=$( [ -e "$SCR/m3c/.git/MERGE_HEAD" ] && echo merge_in_progress || echo no_conflict_state)
b=$(snap "$SCR/m3c"); r=$(hard_update "$SCR/m3c"); a=$(snap "$SCR/m3c")
o="$r"; [ "$b" = "$a" ] && o="$r+unchanged"
rec G03c behaviour REFUSED_INPROGRESS+unchanged "$o" "$(verdict REFUSED_INPROGRESS+unchanged "$o")" "setup=$setup snap $b -> $a"

# G03d diverged history ------------------------------------------------------------------------
need_clone "file://$SCR/s1" "$SCR/m3d" G03d
printf 'local only\n' > "$SCR/m3d/local.txt"; g_hard -C "$SCR/m3d" add local.txt; g_hard -C "$SCR/m3d" commit -q -m local
commit_file s1 c.txt upstream-c
b=$(snap "$SCR/m3d"); r=$(hard_update "$SCR/m3d"); a=$(snap "$SCR/m3d")
o="$r"; [ "$b" = "$a" ] && o="$r+unchanged"
rec G03d behaviour REFUSED_DIVERGED+unchanged "$o" "$(verdict REFUSED_DIVERGED+unchanged "$o")" "snap $b -> $a; $(lastline)"

# G04 template hooks via global init.templateDir -----------------------------------------------
canary_dir G04; home G04; mkdir -p "$SCR/tpl4/hooks"; mk_helper "$SCR/tpl4/hooks/post-checkout" ok
printf '[init]\n\ttemplateDir = %s\n' "$SCR/tpl4" > "$H/.gitconfig"
g_plain clone -q "file://$SCR/s1" "$SCR/pc4"; pc=$(fired); clear_canary
rec G04 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "plain clone with template hook: $pc marker(s)"
hard_clone "file://$SCR/s1" "$SCR/h4"; hcr=$?; hd=$(fired); hk=$( [ -e "$SCR/h4/.git/hooks/post-checkout" ] && echo hook_copied || echo hook_absent)
o=$( [ $hcr -ne 0 ] && echo "clone_aborted:$REASON" || { [ "$hd" -eq 0 ] && echo not_fired || echo fired; })
rec G04 hardened not_fired "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired "$o" || echo inconclusive)" "$hd marker(s); $hk"

# G05 global core.hooksPath --------------------------------------------------------------------
canary_dir G05; home G05; mkdir -p "$SCR/hooks5"; mk_helper "$SCR/hooks5/post-checkout" ok
printf '[core]\n\thooksPath = %s\n' "$SCR/hooks5" > "$H/.gitconfig"
g_plain clone -q "file://$SCR/s1" "$SCR/pc5"; pc=$(fired); clear_canary
rec G05 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "plain clone with global hooksPath: $pc marker(s)"
hard_clone "file://$SCR/s1" "$SCR/h5"; hcr=$?; hd=$(fired); o=$( [ $hcr -ne 0 ] && echo "clone_aborted:$REASON" || { [ "$hd" -eq 0 ] && echo not_fired || echo fired; })
rec G05 hardened not_fired "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired "$o" || echo inconclusive)" "$hd marker(s)"

# G06 hook planted in a managed checkout, triggered by update ----------------------------------
canary_dir G06; H=$HC; need_clone "file://$SCR/s1" "$SCR/m6" G06
mkdir -p -- "$SCR/m6/.git/hooks"; mk_helper "$SCR/m6/.git/hooks/post-merge" ok
inst=$( [ -f "$SCR/m6/.git/hooks/post-merge" ] && [ -x "$SCR/m6/.git/hooks/post-merge" ] && echo installed_executable || echo not_installed)
commit_file s1 d.txt upstream-d; cp -a "$SCR/m6" "$SCR/pc6"
g_plain -C "$SCR/pc6" fetch -q origin; g_plain -C "$SCR/pc6" merge --ff-only refs/remotes/origin/main; pc=$(fired); clear_canary
[ "$inst" = installed_executable ] || pc=0
rec G06 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo inconclusive)" "hook $inst; plain fetch+ff merge with post-merge hook: $pc marker(s)"
r=$(hard_update "$SCR/m6"); hd=$(fired); o=$( [ "$hd" -eq 0 ] && [ "${r%% *}" = UPDATED ] && echo not_fired+updated || echo "fired=$hd/$r")
rec G06 hardened not_fired+updated "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired+updated "$o" || echo inconclusive)" "$r"

# G07 attribute filter (smudge) ----------------------------------------------------------------
canary_dir G07; home G07; H=$HC; mk_src s7
printf '*.dat filter=pwtx\n' > "$SCR/s7/.gitattributes"; printf 'raw-blob\n' > "$SCR/s7/x.dat"
printf 'raw-blob\n' > "$SCR/s7/sp ace.dat"; printf 'raw-blob\n' > "$SCR/s7/nl"$'\n'"name.dat"
g_plain -C "$SCR/s7" add -A; g_plain -C "$SCR/s7" commit -q -m filter
H="$SCR/homes/G07"; mk_helper "$SCR/smudge7" cat
printf '[filter "pwtx"]\n\tsmudge = %s\n\tclean = cat\n' "$SCR/smudge7" > "$H/.gitconfig"
g_plain clone -q "file://$SCR/s7" "$SCR/pc7"; pc=$(fired); clear_canary
rec G07 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "plain clone with smudge filter: $pc marker(s)"
hard_clone "file://$SCR/s7" "$SCR/h7"; hcr=$?; hd=$(fired)
raw=content_raw; for fn in x.dat "sp ace.dat" "nl"$'\n'"name.dat"; do [ "$(cat -- "$SCR/h7/$fn" 2>/dev/null)" = raw-blob ] || raw=content_changed_or_missing; done
o=$( [ $hcr -ne 0 ] && echo "clone_aborted:$REASON" || { [ "$hd" -eq 0 ] && echo "not_fired+$raw" || echo "fired+$raw"; })
rec G07 hardened not_fired+content_raw "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired+content_raw "$o" || echo inconclusive)" "$hd marker(s); verified paths=${VERIFIED_PATHS:-0} (incl. space and newline names)"
commit_file s7 y.dat second; r=$(hard_update "$SCR/h7"); [ "$(cat -- "$SCR/h7/y.dat" 2>/dev/null)" = second ] || r="$r+y.dat_not_raw"; hd=$(fired); o=$( [ "$hd" -eq 0 ] && [ "${r%% *}" = UPDATED ] && echo not_fired+updated || echo "fired=$hd/$r")
rec G07u hardened_update not_fired+updated "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired+updated "$o" || echo inconclusive)" "$r"

# G08 LFS-like long-running process filter (required) ------------------------------------------
canary_dir G08; home G08; H=$HC; mk_src s8
printf '*.bin filter=lfs diff=lfs merge=lfs -text\n' > "$SCR/s8/.gitattributes"; printf 'pointer-like\n' > "$SCR/s8/p.bin"
g_plain -C "$SCR/s8" add .gitattributes p.bin; g_plain -C "$SCR/s8" commit -q -m lfs
H="$SCR/homes/G08"; mk_helper "$SCR/process8" fail
printf '[filter "lfs"]\n\tprocess = %s\n\trequired = true\n' "$SCR/process8" > "$H/.gitconfig"
g_plain clone -q "file://$SCR/s8" "$SCR/pc8"; pc=$(fired); clear_canary
rec G08 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "plain clone with process filter: $pc marker(s); $(lastline)"
hard_clone "file://$SCR/s8" "$SCR/h8"; hc=$?; hd=$(fired)
raw=$( [ "$(cat "$SCR/h8/p.bin" 2>/dev/null)" = pointer-like ] && echo content_raw || echo content_missing)
o=$( [ $hc -ne 0 ] && echo "clone_aborted:$REASON" || { [ "$hd" -eq 0 ] && echo "not_fired+$raw" || echo "fired+$raw"; })
rec G08 hardened not_fired+content_raw "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired+content_raw "$o" || echo inconclusive)" "clone rc=$hc; $hd marker(s)"

# G15 fail-closed regression: controlled, fixture-owned faults in the override setup/verification
canary_dir G15; home G15; mk_helper "$SCR/smudge15" cat
printf '[filter "pwtx"]\n\tsmudge = %s\n\tclean = cat\n' "$SCR/smudge15" > "$H/.gitconfig"
g_plain clone -q "file://$SCR/s7" "$SCR/pc15"; pc=$(fired); clear_canary
rec G15 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "plain clone with G15 smudge helper: $pc marker(s)"
tracked_present() { [ -e "$1/x.dat" ] || [ -e "$1/.gitattributes" ] && echo checked_out || echo not_checked_out; }
for fault in attr_dir_blocked weak_override; do
  hard_clone "file://$SCR/s7" "$SCR/h15-$fault" "$fault"; hcr=$?; hd=$(fired); clear_canary
  o=$( [ $hcr -ne 0 ] && echo "aborted:${REASON%%(*}" || echo proceeded)+$(tracked_present "$SCR/h15-$fault")+$( [ "$hd" -eq 0 ] && echo not_fired || echo fired)
  exp=$( [ $fault = attr_dir_blocked ] && echo aborted:ATTR_DIR_FAILED || echo aborted:ATTR_NOT_UNSET)+not_checked_out+not_fired
  rec "G15-clone-$fault" fail_closed "$exp" "$o" "$( [ "$pc" -gt 0 ] && verdict "$exp" "$o" || echo inconclusive)" "reason=$REASON"
done
for fault in attr_removed weak_override; do
  need_clone "file://$SCR/s7" "$SCR/u15-$fault" "G15-update-$fault" || continue
  commit_file s7 "z-$fault.dat" "new-$fault"
  b=$(snap "$SCR/u15-$fault"); r=$(hard_update "$SCR/u15-$fault" "$fault"); a=$(snap "$SCR/u15-$fault"); hd=$(fired); clear_canary
  o="${r%%:*}+$( [ "$b" = "$a" ] && echo unchanged || echo changed)+$( [ "$hd" -eq 0 ] && echo not_fired || echo fired)"
  exp=$( [ $fault = attr_removed ] && echo REFUSED_ATTR_SETUP || echo REFUSED_ATTR_VERIFY)+unchanged+not_fired
  rec "G15-update-$fault" fail_closed "$exp" "$o" "$( [ "$pc" -gt 0 ] && verdict "$exp" "$o" || echo inconclusive)" "$r; snap $b -> $a"
done

# G09 submodule recursion ----------------------------------------------------------------------
home G09; H=$HC; mk_src s9sub; mk_src s9
g_plain -C "$SCR/s9" -c protocol.file.allow=always submodule add -q "file://$SCR/s9sub" sub; g_plain -C "$SCR/s9" commit -q -m sub
H="$SCR/homes/G09"; printf '[submodule]\n\trecurse = true\n' > "$H/.gitconfig"
g_plain -c protocol.file.allow=always clone -q --recurse-submodules "file://$SCR/s9" "$SCR/pc9"
pc=$( [ -f "$SCR/pc9/sub/a.txt" ] && echo materialised || echo empty)
rec G09 positive_control materialised "$pc" "$(verdict materialised "$pc")" "plain clone --recurse-submodules"
need_clone "file://$SCR/s9" "$SCR/h9" G09
o=$( [ -z "$(ls -A "$SCR/h9/sub" 2>/dev/null)" ] && [ ! -e "$SCR/h9/.git/modules" ] && echo not_recursed || echo recursed)
rec G09 hardened not_recursed "$o" "$( [ "$pc" = materialised ] && verdict not_recursed "$o" || echo inconclusive)" "global submodule.recurse=true present"
commit_file s9 top.txt top-change; r=$(hard_update "$SCR/h9")
o=$( [ "${r%% *}" = UPDATED ] && [ ! -e "$SCR/h9/.git/modules" ] && echo updated+not_recursed || echo "$r")
rec G09u hardened_update updated+not_recursed "$o" "$(verdict updated+not_recursed "$o")" "$r"

# G10 ext:: transport (helper invocation via URL) ----------------------------------------------
canary_dir G10; H=$HC; mk_helper "$SCR/ext10" fail
g_plain -c protocol.ext.allow=always ls-remote "ext::$SCR/ext10"; pc=$(fired); clear_canary
rec G10 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "$pc marker(s); $(lastline)"
g_hard ls-remote "ext::$SCR/ext10"; hd=$(fired); o=$( [ "$hd" -eq 0 ] && echo not_fired || echo fired)
rec G10 hardened not_fired "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired "$o" || echo inconclusive)" "$(lastline)"

# G11 file transport refused without the test-only exception ----------------------------------
H=$HC; g_hard clone -q "file://$SCR/s1" "$SCR/h11"; rc1=$?; l1=$(lastline)
g_hard clone -q "$SCR/s1" "$SCR/h11b"; rc2=$?; l2=$(lastline)
o=$( [ $rc1 -ne 0 ] && echo refused || echo allowed)
rec G11 hardened_no_exception refused "$o" "$(verdict refused "$o")" "file:// rc=$rc1: $l1"
o=$( [ $rc2 -ne 0 ] && echo refused || echo allowed)
rec G11b hardened_no_exception_local_path refused_or_recorded "$o" "$( [ "$o" = refused ] && echo pass || echo observed)" "plain local path rc=$rc2: $l2"

# G12 global core.fsmonitor --------------------------------------------------------------------
canary_dir G12; H=$HC; need_clone "file://$SCR/s1" "$SCR/m12" G12; home G12; mk_helper "$SCR/fsmon12" fail
printf '[core]\n\tfsmonitor = %s\n' "$SCR/fsmon12" > "$H/.gitconfig"
g_plain -C "$SCR/m12" status; pc=$(fired); clear_canary
rec G12 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "plain status: $pc marker(s)"
g_hard -C "$SCR/m12" status; hd=$(fired); o=$( [ "$hd" -eq 0 ] && echo not_fired || echo fired)
rec G12 hardened not_fired "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired "$o" || echo inconclusive)" "$hd marker(s)"

# G13 config injected through inherited environment (GIT_CONFIG_COUNT) --------------------------
canary_dir G13; H=$HC; mk_helper "$SCR/fsmon13" fail
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.fsmonitor GIT_CONFIG_VALUE_0="$SCR/fsmon13" HOME=$HC GIT_CONFIG_NOSYSTEM=1
g_inherit -C "$SCR/m12" status; pc=$(fired); clear_canary
rec G13 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "inherited env injection: $pc marker(s)"
g_hard -C "$SCR/m12" status; hd=$(fired); o=$( [ "$hd" -eq 0 ] && echo not_fired || echo fired)
rec G13 hardened_env_stripped not_fired "$o" "$( [ "$pc" -gt 0 ] && verdict not_fired "$o" || echo inconclusive)" "env -i removes injected config"
g_inherit "${HARD[@]}" -C "$SCR/m12" status; hd=$(fired); clear_canary; o=$( [ "$hd" -eq 0 ] && echo not_fired || echo fired)
rec G13b precedence_observation recorded "$o" observed "injection kept + -c core.fsmonitor=false: $hd marker(s)"
unset GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0

# G14 credential helper preserved (not neutralised) by the hardened options ---------------------
canary_dir G14; home G14; mk_helper "$SCR/cred14" ok
printf '[credential]\n\thelper = %s\n' "$SCR/cred14" > "$H/.gitconfig"
printf 'protocol=https\nhost=example.invalid\n\n' | { logf; base_env timeout 30 "$GIT" credential fill > "$LAST_LOG" 2>&1; }; pc=$(fired); clear_canary
rec G14 positive_control fired "$( [ "$pc" -gt 0 ] && echo fired || echo not_fired)" "$( [ "$pc" -gt 0 ] && echo pass || echo fail)" "git credential fill (local, no network): $pc marker(s)"
g_hard config --get-all credential.helper
o=$(grep -q "cred14" "$LAST_LOG" && echo preserved || echo removed)
rec G14 hardened_config preserved "$o" "$(verdict preserved "$o")" "credential.helper still resolved under hardened -c options; HTTPS invocation not tested (no network)"

# --- Bounds and cleanup ------------------------------------------------------------------------
logbytes=$(du -sb "$SCR/logs" | cut -f1); maxlog=$(find "$SCR/logs" -type f -printf '%s\n' | sort -n | tail -1)
# Process observation using shell builtins only (glob + read), so the observation itself spawns no process.
OBS=; UNEXPECTED=0
for pdir in /proc/[0-9]*; do
  pid=${pdir#/proc/}; stat=
  read -r stat < "$pdir/stat" 2>/dev/null || continue
  comm=${stat#*(}; comm=${comm%)*}; rest=${stat##*) }
  read -r _st ppid pgid _rest <<< "$rest"
  if [ "$pid" = 1 ]; then role=harness; else role=unexpected; UNEXPECTED=$((UNEXPECTED+1)); fi
  OBS+="pid=$pid ppid=$ppid pgid=$pgid comm=$comm role=$role; "
done
rm -rf "$SCR"; gone=$( [ -e "$SCR" ] && echo scratch_present || echo scratch_removed)
rec CLEANUP bounds logs_bounded+scratch_removed+no_unexpected_processes "$( [ "$maxlog" -lt 1048576 ] && echo logs_bounded || echo logs_large)+$gone+$( [ "$UNEXPECTED" -eq 0 ] && echo no_unexpected_processes || echo "unexpected=$UNEXPECTED")" "$( [ "$maxlog" -lt 1048576 ] && [ "$gone" = scratch_removed ] && [ "$UNEXPECTED" -eq 0 ] && echo pass || echo fail)" "total log bytes=$logbytes max=$maxlog; builtin /proc scan (watchdog timeout/unshare are outside this PID namespace): $OBS"
fails=$(cut -f5 "$RESULTS" | grep -cx fail); inc=$(cut -f5 "$RESULTS" | grep -cx inconclusive)
echo "SUMMARY rows=$(wc -l < "$RESULTS") fail=$fails inconclusive=$inc"
[ "$fails" -eq 0 ] && [ "$inc" -eq 0 ]
