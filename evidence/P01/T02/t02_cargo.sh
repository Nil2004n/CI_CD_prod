#!/bin/bash
# P01-T02 lead helper: run one cargo subcommand in the P00/P01 isolated environment inside a network namespace.
set -u
PWT=/home/kali/.local/share/pwt-dev-tools
TC=/home/kali/.rustup/toolchains/stable-x86_64-unknown-linux-gnu
WS="$1"; LOG="$2"; shift 2
ENVV=(PATH="$TC/bin:/usr/bin:/bin" HOME="$PWT/_cargo-home" CARGO_HOME="$PWT/_cargo-home"
      CARGO_TARGET_DIR="$PWT/_cargo-target/p01-lead" RUSTC="$TC/bin/rustc" RUSTDOC="$TC/bin/rustdoc"
      CARGO_TERM_COLOR=never LANG=C.UTF-8)
cd "$WS" || exit 90
unshare -rn env -i "${ENVV[@]}" "$TC/bin/cargo" "$@" > "$LOG" 2>&1
exit $?
