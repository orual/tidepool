# Pre-Publish Plan

Preparing the tidepool workspace for crates.io. The `tidepool` crate name is unclaimed (verified 2026-02-18).

## Decisions

- **License:** MIT OR Apache-2.0 (dual)
- **Tooling:** `cargo-workspaces` for topological publish
- **tidepool-testing:** `publish = false` (internal only)
- **Facade:** `tidepool` crate re-exports everything, no feature gates for 0.1
- **Directories:** full rename alongside crate names
- **Binary distribution:** GitHub release for `tidepool-extract`

## Rename Map

| Current | Target | Dir rename |
|---------|--------|------------|
| `core-repr` | `tidepool-repr` | `core-repr/` → `tidepool-repr/` |
| `core-eval` | `tidepool-eval` | `core-eval/` → `tidepool-eval/` |
| `core-testing` | `tidepool-testing` | `core-testing/` → `tidepool-testing/` |
| `core-heap` | `tidepool-heap` | `core-heap/` → `tidepool-heap/` |
| `core-optimize` | `tidepool-optimize` | `core-optimize/` → `tidepool-optimize/` |
| `core-bridge` | `tidepool-bridge` | `core-bridge/` → `tidepool-bridge/` |
| `core-bridge-derive` | `tidepool-bridge-derive` | `core-bridge-derive/` → `tidepool-bridge-derive/` |
| `core-effect` | `tidepool-effect` | `core-effect/` → `tidepool-effect/` |
| `codegen` | `tidepool-codegen` | `codegen/` → `tidepool-codegen/` |
| `tidepool-macro` | *(no change)* | *(no change)* |
| `tidepool-runtime` | *(no change)* | *(no change)* |
| `tidepool-mcp` | *(no change)* | *(no change)* |

## Steps

### 1. Add license files

- `LICENSE-MIT` at repo root
- `LICENSE-APACHE` at repo root

### 2. Rename directories

`git mv` each directory per the rename map above.

### 3. Update root Cargo.toml

Rename all workspace members to new dir names. Add `tidepool` facade to members.

### 4. Update every crate's Cargo.toml

For each publishable crate:
- `name = "tidepool-*"`
- All `path = "../core-*"` → `path = "../tidepool-*"` (and `../../core-*` in examples)
- Add `version = "0.1.0"` alongside every path dep (required for crates.io)
- Add metadata: `license = "MIT OR Apache-2.0"`, `description`, `repository = "https://github.com/tidepool-heavy-industries/tidepool"`
- Add `publish = false` to: `tidepool-testing`, `examples/guess`, `examples/guess-interpreted`, `examples/tide`, `examples/mcp-server`

### 5. Rename all `use` paths in Rust source

Global find-replace in `.rs` files (not in `.exo/`):

| Old | New |
|-----|-----|
| `core_repr` | `tidepool_repr` |
| `core_eval` | `tidepool_eval` |
| `core_heap` | `tidepool_heap` |
| `core_optimize` | `tidepool_optimize` |
| `core_bridge_derive` | `tidepool_bridge_derive` |
| `core_bridge` | `tidepool_bridge` |
| `core_testing` | `tidepool_testing` |
| `core_effect` | `tidepool_effect` |

Order matters: `core_bridge_derive` before `core_bridge` to avoid partial matches.

For `codegen`: rename `use codegen::` to `use tidepool_codegen::` in examples and codegen's own test files.

### 6. Update quote! macro paths in proc-macro crates

`core-bridge-derive/src/codegen.rs` and `tidepool-macro/src/expand.rs` emit crate paths in generated code via `quote!`. All `core_repr::`, `core_eval::`, `core_bridge::` in token streams must become `tidepool_*::`.

### 7. Create facade crate

New `tidepool/` directory:
- `Cargo.toml` — depends on all publishable crates, full metadata
- `src/lib.rs` — re-exports public API with `//!` crate doc

### 8. GitHub Actions: release workflow

`.github/workflows/release.yml`:
- Trigger on tag push `v*`
- Build `tidepool-extract` via `nix build .#tidepool-extract`
- Create GitHub release, attach binary
- Run `cargo ws publish --from-git --yes`

## Verify

```bash
cargo install cargo-workspaces
cargo check --workspace
cargo test --workspace
cargo ws publish --dry-run
```
