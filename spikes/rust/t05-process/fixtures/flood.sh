# P00-T05 synthetic fixture (flood.sh). Spike-owned; run only by the t05-process harness via /bin/sh.
if [ "$1" = err ]; then yes PWT-FLOOD-LINE >&2; else yes PWT-FLOOD-LINE; fi
