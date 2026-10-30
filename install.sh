#!/bin/sh
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
for binary in "$here/herdr-auto-titles" "$here/target/release/herdr-auto-titles"; do
  if [ -x "$binary" ]; then exec "$binary" install "$@"; fi
done
echo "Download the native release or run cargo build --release --locked, then retry." >&2
exit 1
