# P00-T05 synthetic fixture (grandchild.sh). Spike-owned; run only by the t05-process harness via /bin/sh.
sleep 1000 &
echo "grandchild_pid=$!"
exit 0
