#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
oxlint_bin=${OXLINT_BIN:-"$repo_dir/tests/ui/node_modules/.bin/oxlint"}

if [ ! -x "$oxlint_bin" ] && ! command -v "$oxlint_bin" >/dev/null 2>&1; then
    echo "Install UI development dependencies with: npm --prefix tests/ui ci" >&2
    echo "Or set OXLINT_BIN to a compatible standalone Oxlint executable." >&2
    exit 1
fi

cd "$repo_dir"
exec "$oxlint_bin" --config .oxlintrc.json --max-warnings 0 assets/js scripts/minify-js.cjs tests/ui/read-script.cjs tests/ui/minified-bridges.test.cjs "$@"
