#!/bin/sh
# PWT MCP stdio launcher: single resolution path for the Java runtime and the
# built application artifact. Used by coding-client MCP configurations and by
# `make mcp`, so all entrypoints launch the same server the same way.
#
# Usage: pwt-mcp-stdio.sh [--data-dir DIR] [--allow-scans] [--jar PATH] [--java BIN]
#
# Resolution order:
#   Java: --java, then $JAVA_HOME/bin/java, then a validated `java` from PATH.
#   JAR:  --jar, else the newest pwt/target/pwt-*.jar next to this repository
#         layout, excluding original-*, *sources*, *javadoc*, *tests* builds.
#         Zero or ambiguous matches are an error (listed, not guessed).
# Data dir defaults to ~/.pwt-data. Diagnostics on stderr; stdout stays
# MCP-protocol-only (this script prints nothing on stdout before exec).
set -u

DATA_DIR="$HOME/.pwt-data"
ALLOW_SCANS=""
JAR_ARG=""
JAVA_ARG=""

while [ $# -gt 0 ]; do
  case "$1" in
    --data-dir) DATA_DIR="$2"; shift 2;;
    --allow-scans) ALLOW_SCANS="--allow-scans"; shift 1;;
    --jar) JAR_ARG="$2"; shift 2;;
    --java) JAVA_ARG="$2"; shift 2;;
    -h|--help) sed -n '2,16p' "$0"; exit 0;;
    *) echo "pwt-mcp-stdio: unknown argument: $1" >&2; exit 2;;
  esac
done

if [ -n "$JAVA_ARG" ]; then
  JAVA_BIN="$JAVA_ARG"
elif [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/java" ]; then
  JAVA_BIN="$JAVA_HOME/bin/java"
else
  JAVA_BIN="$(command -v java 2>/dev/null || true)"
fi
if [ -z "$JAVA_BIN" ]; then
  echo "pwt-mcp-stdio: no Java executable found (pass --java, set JAVA_HOME, or put java on PATH)" >&2
  exit 2
fi
if ! "$JAVA_BIN" -version >/dev/null 2>&1; then
  echo "pwt-mcp-stdio: Java executable does not run: $JAVA_BIN" >&2
  exit 2
fi

if [ -n "$JAR_ARG" ]; then
  JAR="$JAR_ARG"
else
  SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
  CANDIDATES=""
  COUNT=0
  for candidate in "$SCRIPT_DIR"/../pwt/target/pwt-*.jar; do
    case "$candidate" in
      *original-*|*sources*|*javadoc*|*tests*) continue;;
    esac
    if [ -f "$candidate" ]; then
      CANDIDATES="$CANDIDATES $candidate"
      COUNT=$((COUNT + 1))
    fi
  done
  if [ "$COUNT" -eq 0 ]; then
    echo "pwt-mcp-stdio: no application artifact found under $SCRIPT_DIR/../pwt/target (run make build; or pass --jar)" >&2
    exit 2
  fi
  if [ "$COUNT" -gt 1 ]; then
    # shellcheck disable=SC2086
    set -- $CANDIDATES
    JAR=""
    for candidate in "$@"; do
      if [ -z "$JAR" ] || [ "$candidate" -nt "$JAR" ]; then
        JAR="$candidate"
      fi
    done
    echo "pwt-mcp-stdio: note: multiple artifacts, using newest: $JAR" >&2
  else
    # shellcheck disable=SC2086
    set -- $CANDIDATES
    JAR="$1"
  fi
fi
if [ ! -f "$JAR" ]; then
  echo "pwt-mcp-stdio: JAR not found: $JAR" >&2
  exit 2
fi

mkdir -p "$DATA_DIR" 2>/dev/null || {
  echo "pwt-mcp-stdio: cannot use data directory: $DATA_DIR" >&2
  exit 2
}

# Fixed argument array form (every expansion quoted; no eval, no word splitting).
if [ -n "$ALLOW_SCANS" ]; then
  exec "$JAVA_BIN" -jar "$JAR" mcp --data-dir "$DATA_DIR" --allow-scans
else
  exec "$JAVA_BIN" -jar "$JAR" mcp --data-dir "$DATA_DIR"
fi
