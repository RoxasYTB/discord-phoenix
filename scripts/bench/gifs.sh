#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
dir=${1:-$(mktemp -d)}
[[ -s $dir/ids.txt ]] || { echo "need $dir/ids.txt"; exit 1; }
for v in C d M; do
  while read -r l; do
    id=${l%%/*}; slug=${l#*/}; f=$dir/$v-$id.gif
    [[ -s $f ]] || curl -s -o "$f" "https://media.tenor.com/${id}AAAA$v/${slug}gif"
  done <"$dir/ids.txt"
done
target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')
nice -n 5 cargo build -q -p phoenix-app --example gif_mem
for v in C d M; do "$target/debug/examples/gif_mem" "$dir" "$v-"; done
