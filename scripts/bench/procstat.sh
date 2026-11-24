#!/usr/bin/env bash
set -euo pipefail
secs=$1; shift
pids=("$@")
hz=$(getconf CLK_TCK)
ticks() {
  local t=0 p
  for p in "${pids[@]}"; do
    [[ -r /proc/$p/stat ]] || continue
    t=$(( t + $(sed 's/^.*) //' /proc/$p/stat | awk '{print $12+$13}') ))
  done
  echo $t
}
t0=$(ticks); s0=$(date +%s.%N)
sleep "$secs"
t1=$(ticks); s1=$(date +%s.%N)
rss=0; pss=0; n=0
for p in "${pids[@]}"; do
  f=/proc/$p/smaps_rollup
  [[ -r $f ]] || continue
  n=$((n+1))
  rss=$(( rss + $(awk '/^Rss:/{print $2}' $f) ))
  pss=$(( pss + $(awk '/^Pss:/{print $2}' $f) ))
done
awk -v n=$n -v r=$rss -v p=$pss -v dt="$(echo "$s1 - $s0" | bc)" -v dk=$((t1-t0)) -v hz=$hz \
  'BEGIN{printf "procs=%d rss_mib=%.1f pss_mib=%.1f cpu_pct=%.2f\n", n, r/1024, p/1024, 100*dk/hz/dt}'
