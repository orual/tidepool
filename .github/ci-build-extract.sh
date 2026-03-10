#!/usr/bin/env bash
# Build tidepool-extract with persistent cabal cache and emit a wrapper script.
# Usage: source <(./ci-build-extract.sh)
#   This prints PATH=... to stdout; sourcing it adds tidepool-extract to PATH.
set -euo pipefail

CABAL_STORE="${CABAL_STORE:-$HOME/.local/state/cabal/store}"
CABAL_BUILDDIR="${CABAL_BUILDDIR:-/var/lib/github-runner/tidepool/cache/cabal-dist}"

cd "$(dirname "$0")/../haskell"

cabal --store-dir="$CABAL_STORE" update
cabal --store-dir="$CABAL_STORE" build exe:tidepool-extract-bin \
  --builddir="$CABAL_BUILDDIR"

BIN=$(cabal --store-dir="$CABAL_STORE" list-bin tidepool-extract-bin \
  --builddir="$CABAL_BUILDDIR")

GHC_DIR=$(dirname "$(which ghc)")

WRAPPER_DIR=$(mktemp -d)
cat > "$WRAPPER_DIR/tidepool-extract" <<EOF
#!/usr/bin/env bash
export PATH="$GHC_DIR:\$PATH"
exec "$BIN" "\$@"
EOF
chmod +x "$WRAPPER_DIR/tidepool-extract"

echo "$WRAPPER_DIR"
