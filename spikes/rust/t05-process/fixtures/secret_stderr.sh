# P00-T05 synthetic fixture (secret_stderr.sh). Spike-owned; run only by the t05-process harness via /bin/sh.
printf "api_token=PWTFAKE_C10_%s\n" "$(od -An -N12 -tx1 /dev/urandom | tr -d " \n")" >&2
echo "done"
