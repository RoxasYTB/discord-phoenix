#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
here=scripts/bench
target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')
bin=$target/release/discord-phoenix
window=1; build=1
for a in "$@"; do case $a in --no-window) window=0;; --skip-build) build=0;; esac; done

if (( build )); then
  nice -n 19 cargo rustc --release -p phoenix-app --bin discord-phoenix -- -L "$HOME/.local/lib/devlinks"
  nice -n 19 cargo build --release -p phoenix-app --example bench
fi
echo "== hardware"
echo "cpu=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs) ($(nproc) threads)"
echo "gpu=$(lspci | grep -iE 'vga|3d' | cut -d: -f3 | xargs)"
echo "ram_gib=$(awk '/MemTotal/{printf "%.0f", $2/1048576}' /proc/meminfo)"
echo "kernel=$(uname -r) session=${XDG_SESSION_TYPE:-?}"
echo "== binary"
echo "binary_bytes=$(stat -c %s "$bin")"
echo "binary_mib=$(awk -v b="$(stat -c %s "$bin")" 'BEGIN{printf "%.1f", b/1048576}')"

echo "== headless (mock, 5000 messages)"
nice -n 5 "$target/release/examples/bench" 5000

if (( window )); then
  echo "== real window (mock, auto-login, quits after 40 s)"
  log=$(mktemp)
  start=$(date +%s%3N)
  PHOENIX_MOCK=1 PHOENIX_MOCK_AUTOLOGIN=1 PHOENIX_BENCH=1 PHOENIX_BENCH_EXIT_MS=40000 "$bin" 2>"$log" &
  pid=$!
  for _ in $(seq 100); do grep -q first_frame "$log" && break; sleep 0.1; done
  ff=$(grep -o 'first_frame_epoch_ms=[0-9]*' "$log" | cut -d= -f2 || true)
  [[ -n $ff ]] && echo "cold_start_first_frame_ms=$((ff - start))"
  sleep 8
  echo "idle after settle (30 s sample, total ~38 s after start):"
  "$here/procstat.sh" 30 "$pid"
  wait "$pid" || true
  rm -f "$log"
fi

if pgrep -f 'discord/app-' >/dev/null; then
  echo "== official Discord (read-only, 30 s)"
  "$here/discord_official.sh" 30
fi
