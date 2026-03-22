#!/usr/bin/env bash
# Generate a cross-language token verification fixture.
# Runs the Rust test that outputs a fixture JSON to stdout.
set -euo pipefail

cd "$(dirname "$0")/../.."
cargo test -p cloak-tokens -- --ignored --nocapture test_cross_language_fixture 2>/dev/null \
  | grep '^{' > tests/cross-language/fixture.json

echo "Fixture written to tests/cross-language/fixture.json"
cat tests/cross-language/fixture.json
