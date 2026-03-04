# Publishing to crates.io

## Publish Order

Crates must be published in dependency order. Wait for each crate to appear on crates.io before publishing the next.

```
1.  tidepool-repr
2.  tidepool-eval
3.  tidepool-heap
4.  tidepool-bridge
5.  tidepool-bridge-derive
6.  tidepool-optimize
7.  tidepool-macro
8.  tidepool-effect
9.  tidepool-codegen
10. tidepool-runtime
11. tidepool-mcp
12. tidepool (binary)
```

`tidepool-testing` has `publish = false` and is a dev-dependency only — crates.io ignores it.

Note: v0.0.1 was previously published for all crates, so 0.1.0 will work.

## Dry Run

```bash
cargo publish --dry-run -p tidepool-repr
cargo publish --dry-run -p tidepool-eval
# ... etc
```

## Publish

```bash
cargo publish -p tidepool-repr
# wait for it to appear on crates.io
cargo publish -p tidepool-eval
# ... continue in order
```

## Cachix Binary Cache

Push Nix build artifacts to the `tidepool` Cachix cache for both Linux x86_64 and macOS aarch64.

### Setup

```bash
# Install cachix (if not present)
nix-env -iA cachix -f https://cachix.org/api/v1/install
# or: nix profile install nixpkgs#cachix

# Auth (needs token from https://app.cachix.org)
cachix authtoken <TOKEN>
```

### Push

```bash
# Build and push tidepool-extract
nix build .#tidepool-extract
cachix push tidepool $(nix build .#tidepool-extract --print-out-paths)

# Also push the dev shell closure
nix build .#devShells.$(nix eval --raw 'nixpkgs#system').default
cachix push tidepool $(nix build .#devShells.$(nix eval --raw 'nixpkgs#system').default --print-out-paths)
```

Run on both Linux x86_64 and macOS aarch64 to populate the cache for both architectures.
