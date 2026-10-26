#!/bin/sh
# SPDX-License-Identifier: MIT
# Manual native packaging entry point; performs no install, upload, or CI dispatch.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if ! command -v node >/dev/null 2>&1; then
  printf '%s\n' 'Node.js 24 is required for the packaging tool.' >&2
  exit 1
fi
exec node "$script_dir/package-unix.mjs" "$@"
