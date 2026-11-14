#!/bin/sh
# PWT MCP stdio launcher: single resolution path for the Java runtime and the
# built application artifact. Used by coding-client MCP configurations and by
# `make mcp`, so all entrypoints launch the same server the same way.
#
# Usage: pwt-mcp-stdio.sh [--data-dir DIR] [--allow-scans] [--allow-analysis-writes]
#                        [--repo-id ID] [--jar PATH] [--java BIN] [--print-jar]
#
# --print-jar prints the resolved artifact path and exits (used by the
# OpenCode dashboard launcher to find the GUI jar without duplicating
# resolution logic).
#
# Resolution order:
#   Java: --java, then $JAVA_HOME/bin/java, then a validated `java` from PATH.
#   JAR:  --jar, else the newest pwt/target/pwt-*.jar next to this repository
#         layout, excluding original-*, *sources*, *javadoc*, *tests* builds.
#         Zero or ambiguous matches are an error (listed, not guessed).
#   Data dir: --data-dir, else <application-root>/.pwt-data (one shared
#         database for every repository and scan).
# Data dir defaults to <application-root>/.pwt-data, where the application
# root is the directory above integrations/ (the one holding the root
# Makefile). Every repository and scan shares this one database; project
# scoping filters records, it never selects another database file.
# Diagnostics on stderr; stdout stays
# MCP-protocol-only (this script prints nothing on stdout before exec).
set -u

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
APP_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
DATA_DIR="$APP_ROOT/.pwt-data"
ALLOW_SCANS=""
ALLOW_ANALYSIS=""
REPO_ID=""
JAR_ARG=""
JAVA_ARG=""

while [ $# -gt 0 ]; do
  case "$1" in
    --data-dir) DATA_DIR="$2"; shift 2;;
    --allow-scans) ALLOW_SCANS="--allow-scans"; shift 1;;
    --allow-analysis-writes) ALLOW_ANALYSIS="--allow-analysis-writes"; shift 1;;
    --repo-id) REPO_ID="$2"; shift 2;;
    --jar) JAR_ARG="$2"; shift 2;;
    --java) JAVA_ARG="$2"; shift 2;;
    --print-jar) PRINT_JAR=1; shift 1;;
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
if [ "${PRINT_JAR:-}" = "1" ]; then
  printf '%s\n' "$JAR"
  exit 0
fi

mkdir -p "$DATA_DIR" 2>/dev/null || {
  echo "pwt-mcp-stdio: cannot use data directory: $DATA_DIR" >&2
  exit 2
}

# Fixed argument list built with positional parameters (no eval, no word
# splitting; values stay quoted at every expansion).
set --
if [ -n "$ALLOW_SCANS" ]; then
  set -- "$@" --allow-scans
fi
if [ -n "$ALLOW_ANALYSIS" ]; then
  set -- "$@" --allow-analysis-writes
fi
if [ -n "$REPO_ID" ]; then
  set -- "$@" --repo-id "$REPO_ID"
fi
exec "$JAVA_BIN" -jar "$JAR" mcp --data-dir "$DATA_DIR" "$@"
