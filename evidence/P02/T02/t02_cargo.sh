#!/bin/bash
# T02 helper: runs one cargo command in the P01/P02 environment inside a fresh network namespace.
set -u
PWT=$HOME/.local/share/pwt-dev-tools; TC=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin
envv=(PATH=$TC:/usr/bin:/bin HOME=$PWT/_cargo-home CARGO_HOME=$PWT/_cargo-home CARGO_TARGET_DIR=$PWT/_cargo-target/p02-t02 RUSTC=$TC/rustc RUSTDOC=$TC/rustdoc CARGO_TERM_COLOR=never LANG=C.UTF-8)
exec unshare -rn env -i "${envv[@]}" "$TC/cargo" "$@"
