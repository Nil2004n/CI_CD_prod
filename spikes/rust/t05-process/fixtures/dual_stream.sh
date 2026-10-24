# P00-T05 synthetic fixture (dual_stream.sh). Spike-owned; run only by the t05-process harness via /bin/sh.
head -c 524288 /dev/zero | tr '\000' o &
head -c 524288 /dev/zero | tr '\000' e >&2
wait
