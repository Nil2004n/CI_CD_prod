#!/bin/sh
# PWT dashboard launcher: opens the existing JavaFX application on the user's
# local graphical machine. Narrow integration action (window launch only);
# all assessment work goes through MCP, never through this script.
#
# Usage: pwt-dashboard.sh [--jar /abs/path/pwt-X.jar] [--data-dir DIR] [--java BIN] [--detach]
#
# - Without --jar (and without PWT_JAR) the artifact is resolved by the
#   sibling pwt-mcp-stdio.sh --print-jar (same rules as every entrypoint).
# - --detach starts the window in the background and returns immediately
#   (used by client commands that must not block on the GUI).
# - No shell-string command construction: fixed argument array below.
# - Refuses to start without a graphical session (SSH/container).
# - Opens at most one window: if a PWT GUI is already running, reports its
#   pid and exits 0 without launching another.
set -u

JAR=""
# One shared database: <application-root>/.pwt-data unless --data-dir says
# otherwise explicitly. Never generated per repository or scan.
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
DATA_DIR="$SCRIPT_DIR/../.pwt-data"
JAVA_BIN="java"
DETACH=""

while [ $# -gt 0 ]; do
  case "$1" in
    --jar) JAR="$2"; shift 2;;
    --data-dir) DATA_DIR="$2"; shift 2;;
    --java) JAVA_BIN="$2"; shift 2;;
    --detach) DETACH=1; shift 1;;
    -h|--help) sed -n '2,17p' "$0"; exit 0;;
    *) echo "pwt-dashboard: unknown argument: $1" >&2; exit 2;;
  esac
done

if [ -z "$JAR" ] && [ -n "${PWT_JAR:-}" ]; then
  JAR="$PWT_JAR"
fi
if [ -z "$JAR" ] && [ -x "$SCRIPT_DIR/pwt-mcp-stdio.sh" ]; then
  JAR="$("$SCRIPT_DIR/pwt-mcp-stdio.sh" --java "$JAVA_BIN" --print-jar 2>/dev/null || true)"
fi
if [ -z "$JAR" ]; then
  echo "pwt-dashboard: no JAR configured (pass --jar, set PWT_JAR, or run make build)" >&2
  exit 2
fi
if [ ! -f "$JAR" ]; then
  echo "pwt-dashboard: JAR not found: $JAR" >&2
  exit 2
fi
if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "pwt-dashboard: no graphical session detected (DISPLAY and WAYLAND_DISPLAY are unset)." >&2
  echo "The dashboard can only open on a machine with a display. Run this helper there," >&2
  echo "or inspect results headlessly with: java -jar $JAR scan --help" >&2
  exit 2
fi

# Single instance: do not stack windows. The bracketed character class keeps
# this pgrep from matching helper invocations or callers that merely mention
# the pattern text.
if command -v pgrep >/dev/null 2>&1; then
  EXISTING="$(pgrep -f "pwt-.*\.jar[ ]gu[i]" 2>/dev/null | head -1)"
  if [ -n "$EXISTING" ]; then
    echo "pwt-dashboard: a PWT window is already running (pid $EXISTING); not opening another."
    exit 0
  fi
fi

mkdir -p "$DATA_DIR" 2>/dev/null || {
  echo "pwt-dashboard: cannot use data directory: $DATA_DIR" >&2
  exit 2
}

# Fixed argument array form: "$JAVA_BIN" "-jar" "$JAR" "gui" "--data-dir" "$DATA_DIR"
# (word-splitting safe: every expansion is quoted; no eval.)
if [ -n "$DETACH" ]; then
  if command -v setsid >/dev/null 2>&1; then
    setsid "$JAVA_BIN" "-jar" "$JAR" "gui" "--data-dir" "$DATA_DIR" </dev/null >/dev/null 2>&1 &
  else
    nohup "$JAVA_BIN" "-jar" "$JAR" "gui" "--data-dir" "$DATA_DIR" </dev/null >/dev/null 2>&1 &
  fi
  echo "pwt-dashboard: dashboard launch requested (pid $!); data directory: $DATA_DIR"
  exit 0
fi
exec "$JAVA_BIN" "-jar" "$JAR" "gui" "--data-dir" "$DATA_DIR"
