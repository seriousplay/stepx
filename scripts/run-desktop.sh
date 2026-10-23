#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$ROOT/.tools/bin:$PATH"
cd "$ROOT/desktop"
exec cargo run --bin stepx-companion -- "$@"
