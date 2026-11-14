#!/bin/sh
# PWT user-wide entrypoint. Thin wrapper: resolves the installation, picks
# Java and the built artifact with the same rules as pwt-mcp-stdio.sh, then
# execs the requested mode. All arguments are passed through verbatim
# (quoted); the data directory resolves via PWT_DATA_DIR (baked in by
# `make install`) or <application-root>/.pwt-data, and any explicit
# --data-dir flag still wins because every mode parses it directly.
#
# Usage: pwt <mode> [options]
#   Modes: gui | discover | scan | mcp | repo-add | repo-list | doctor |
#          mcp-status | journal | kubeconform-setup | --help | --version
#   Examples:
#     pwt gui
#     pwt scan --root /abs/project --file a.yaml --adapter yamllint,gitleaks
#     pwt repo-add --root /abs/project
#     pwt doctor
# Diagnostics go to stderr; stdout is untouched (MCP protocol safety).
set -u

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
APP_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"

if [ -n "${PWT_JAVA:-}" ]; then
  JAVA_BIN="$PWT_JAVA"
elif [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/java" ]; then
  JAVA_BIN="$JAVA_HOME/bin/java"
else
  JAVA_BIN="$(command -v java 2>/dev/null || true)"
fi
if [ -z "$JAVA_BIN" ]; then
  echo "pwt: no Java executable found (set PWT_JAVA or JAVA_HOME, or put java on PATH)" >&2
  exit 2
fi
if ! "$JAVA_BIN" -version >/dev/null 2>&1; then
  echo "pwt: Java executable does not run: $JAVA_BIN" >&2
  exit 2
fi

if [ -n "${PWT_JAR:-}" ] && [ -f "$PWT_JAR" ]; then
  JAR="$PWT_JAR"
else
  JAR=""
  for candidate in "$APP_ROOT"/pwt/target/pwt-*.jar; do
    case "$candidate" in
      *original-*|*sources*|*javadoc*|*tests*) continue;;
    esac
    if [ -f "$candidate" ]; then
      if [ -z "$JAR" ]; then
        JAR="$candidate"
      else
        echo "pwt: ambiguous application artifact (expected exactly one):" >&2
        echo "pwt:   $JAR" >&2
        echo "pwt:   $candidate" >&2
        echo "pwt: pass PWT_JAR=/abs/path/pwt-X.jar to select one" >&2
        exit 2
      fi
    fi
  done
  if [ -z "$JAR" ]; then
    echo "pwt: no built artifact found under $APP_ROOT/pwt/target (run: make build)" >&2
    exit 2
  fi
fi

exec "$JAVA_BIN" -jar "$JAR" "$@"
