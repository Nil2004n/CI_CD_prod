#!/bin/sh
# PWT MCP stdio launcher: single resolution path for the Java runtime and the
# built application artifact. Used by coding-client MCP configurations and by
# `make mcp`, so all entrypoints launch the same server the same way.
#
# Usage: pwt-mcp-stdio.sh [--data-dir DIR] [--allow-scans] [--allow-analysis-writes]
#                        [--repo-id ID | --project-root ABS_DIR | --project-root-cwd]
#                        [--jar PATH] [--java BIN] [--print-jar]
#
# --project-root-cwd binds the server to the directory it was started in
# (pwd -P), for user-wide client entries whose client starts the server in
# the session's project directory (Claude Code does).
#
# --print-jar prints the resolved artifact path and exits (used by the
# OpenCode dashboard launcher to find the GUI jar without duplicating
# resolution logic).
#
# Resolution order:
#   Java: --java, then $JAVA_HOME/bin/java, then a validated `java` from PATH.
#   JAR:  --jar, else PWT_JAR, else pwt/target/pwt-<pom version>.jar, else the
#         single pwt/target/pwt-*.jar (original-*, *sources*, *javadoc*,
#         *tests* excluded). Zero or ambiguous matches are an error (listed,
#         not guessed).
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
PROJECT_ROOT=""
PROJECT_ROOT_CWD=""
JAR_ARG=""
JAVA_ARG=""

while [ $# -gt 0 ]; do
  case "$1" in
    --data-dir) DATA_DIR="$2"; shift 2;;
    --allow-scans) ALLOW_SCANS="--allow-scans"; shift 1;;
    --allow-analysis-writes) ALLOW_ANALYSIS="--allow-analysis-writes"; shift 1;;
    --repo-id) REPO_ID="$2"; shift 2;;
    --project-root) PROJECT_ROOT="$2"; shift 2;;
    --project-root-cwd) PROJECT_ROOT_CWD=1; shift 1;;
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

# Artifact selection (one policy for every entrypoint; pwt.sh and the
# dashboard helper resolve through --print-jar):
#   1. --jar, else PWT_JAR;
#   2. else pwt/target/pwt-<version>.jar for the version declared in
#      pwt/pom.xml (the same rule the Makefile builds with);
#   3. else exactly one pwt/target/pwt-*.jar (original-*, sources, javadoc,
#      tests excluded; renamed copies such as *.jar.inuse never match);
#   4. else fail and list the candidates. Never "newest wins".
if [ -n "$JAR_ARG" ]; then
  JAR="$JAR_ARG"
elif [ -n "${PWT_JAR:-}" ]; then
  JAR="$PWT_JAR"
else
  SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
  APP_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
  TARGET_DIR="$APP_DIR/pwt/target"
  JAR=""
  POM="$APP_DIR/pwt/pom.xml"
  if [ -f "$POM" ]; then
    DECLARED="$(grep -m1 '<version>' "$POM" | sed -e 's/.*<version>//' -e 's/<\/version.*//')"
    if [ -n "$DECLARED" ] && [ -f "$TARGET_DIR/pwt-$DECLARED.jar" ]; then
      JAR="$TARGET_DIR/pwt-$DECLARED.jar"
    fi
  fi
  if [ -z "$JAR" ]; then
    CANDIDATES=""
    COUNT=0
    for candidate in "$TARGET_DIR"/pwt-*.jar; do
      case "$candidate" in
        *original-*|*sources*|*javadoc*|*tests*) continue;;
      esac
      if [ -f "$candidate" ]; then
        CANDIDATES="$CANDIDATES
  $candidate"
        COUNT=$((COUNT + 1))
        JAR="$candidate"
      fi
    done
    if [ "$COUNT" -eq 0 ]; then
      echo "pwt-mcp-stdio: no application artifact found under $TARGET_DIR (run make build; or pass --jar)" >&2
      exit 2
    fi
    if [ "$COUNT" -gt 1 ]; then
      echo "pwt-mcp-stdio: ambiguous application artifacts and none matches the declared version${DECLARED:+ $DECLARED}:$CANDIDATES" >&2
      echo "pwt-mcp-stdio: run make build, or pass --jar / set PWT_JAR to select one" >&2
      exit 2
    fi
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
if [ -n "$PROJECT_ROOT_CWD" ]; then
  if [ -n "$PROJECT_ROOT" ]; then
    echo "pwt-mcp-stdio: use either --project-root or --project-root-cwd, not both" >&2
    exit 2
  fi
  PROJECT_ROOT="$(pwd -P)"
fi
if [ -n "$PROJECT_ROOT" ]; then
  set -- "$@" --project-root "$PROJECT_ROOT"
fi
exec "$JAVA_BIN" -jar "$JAR" mcp --data-dir "$DATA_DIR" "$@"
