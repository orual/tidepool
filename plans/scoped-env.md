# Plan: ScopedEnv + EnvScope

Replace `HashMap<VarId, SsaVal>` with a `ScopedEnv` newtype that has no public `remove`, and add an `EnvScope` helper for batch save/restore. This makes it structurally impossible to forget restoring env entries after temporary bindings.

## Motivation

The save/restore pattern is copy-pasted in 4 places (case binder, pattern vars, LetCleanup, join params). Each site has the same 5-line if/else block. Every new binding site is a potential bug — the previous bug was exactly a bare `env.remove` that should have been a save/restore.

## New types (in `emit/mod.rs`)

```rust
/// Environment mapping VarIds to SSA values.
/// No public `remove` — callers must go through `restore` to undo bindings.
pub struct ScopedEnv {
    inner: HashMap<VarId, SsaVal>,
}

impl ScopedEnv {
    pub fn new() -> Self { Self { inner: HashMap::new() } }

    pub fn get(&self, var: &VarId) -> Option<&SsaVal> {
        self.inner.get(var)
    }

    pub fn contains_key(&self, var: &VarId) -> bool {
        self.inner.contains_key(var)
    }

    /// Insert a binding, returning the old value (if any) for later restore.
    pub fn insert(&mut self, var: VarId, val: SsaVal) -> Option<SsaVal> {
        self.inner.insert(var, val)
    }

    /// Undo a binding: restore the old value, or remove if there was none.
    pub fn restore(&mut self, var: VarId, old: Option<SsaVal>) {
        match old {
            Some(v) => { self.inner.insert(var, v); }
            None => { self.inner.remove(&var); }
        }
    }

    /// Iterate over all entries (for declare_env, compute_captures, etc.)
    pub fn iter(&self) -> impl Iterator<Item = (&VarId, &SsaVal)> {
        self.inner.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &VarId> {
        self.inner.keys()
    }
}
```

```rust
/// Batch save/restore scope. Collects (VarId, old_value) pairs.
/// Call `env.restore_scope(scope)` to undo all bindings in reverse order.
pub struct EnvScope {
    saved: Vec<(VarId, Option<SsaVal>)>,
}

impl EnvScope {
    pub fn new() -> Self { Self { saved: Vec::new() } }
}

impl ScopedEnv {
    /// Insert within a scope — automatically records the old value for restore.
    pub fn insert_scoped(&mut self, scope: &mut EnvScope, var: VarId, val: SsaVal) {
        let old = self.inner.insert(var, val);
        scope.saved.push((var, old));
    }

    /// Restore all bindings recorded in this scope, in reverse order.
    pub fn restore_scope(&mut self, scope: EnvScope) {
        for (var, old) in scope.saved.into_iter().rev() {
            self.restore(var, old);
        }
    }
}
```

## Changes by file

### `emit/mod.rs`
- Add `ScopedEnv` and `EnvScope` types (above)
- Change `EmitContext.env` from `HashMap<VarId, SsaVal>` to `ScopedEnv`
- Update `EmitContext::new` to use `ScopedEnv::new()`
- Update `declare_env` to use `self.env.keys()` / `self.env.get()`

### `emit/case.rs`
- `emit_case` (line 24, 86-90): Replace manual save/restore with `insert` + `restore`
  ```rust
  // Before:
  let old_case_binder = ctx.env.insert(*binder, scrut);
  // ... work ...
  if let Some(v) = old_case_binder {
      ctx.env.insert(*binder, v);
  } else {
      ctx.env.remove(binder);
  }

  // After:
  let old = ctx.env.insert(*binder, scrut);
  // ... work ...
  ctx.env.restore(*binder, old);
  ```

- `emit_data_dispatch` (line 186-215): Replace manual vec + if/else loop with `EnvScope`
  ```rust
  // Before:
  let mut old_bound_vals: Vec<(VarId, Option<SsaVal>)> = Vec::new();
  for (i, &binder) in alt.binders.iter().enumerate() {
      let old_val = ctx.env.insert(binder, SsaVal::HeapPtr(field_val));
      old_bound_vals.push((binder, old_val));
  }
  // ... work ...
  for (binder, old_val) in old_bound_vals {
      if let Some(v) = old_val { ctx.env.insert(binder, v); }
      else { ctx.env.remove(&binder); }
  }

  // After:
  let mut scope = EnvScope::new();
  for (i, &binder) in alt.binders.iter().enumerate() {
      ctx.env.insert_scoped(&mut scope, binder, SsaVal::HeapPtr(field_val));
  }
  // ... work ...
  ctx.env.restore_scope(scope);
  ```

### `emit/join.rs`
- `emit_join` (line 56-88): Replace manual vec + if/else loop with `EnvScope`
  ```rust
  // After:
  let mut scope = EnvScope::new();
  for (i, param_var) in params.iter().enumerate() {
      ctx.env.insert_scoped(&mut scope, *param_var, SsaVal::HeapPtr(val));
  }
  // ... work ...
  ctx.env.restore_scope(scope);
  ```

### `emit/expr.rs`
- `LetCleanup` enum: Change to use `EnvScope` instead of ad-hoc vecs
  ```rust
  enum LetCleanup {
      Single(VarId, Option<SsaVal>),
      Rec(EnvScope),
  }
  ```
- `LetCleanupMark` handler (line 1309-1327): Replace if/else with `restore`/`restore_scope`
  ```rust
  LetCleanup::Single(var, old) => { self.env.restore(var, old); }
  LetCleanup::Rec(scope) => { self.env.restore_scope(scope); }
  ```
- LetNonRec push site (line 1226, 1235): Use `insert` + pass old to `LetCleanup::Single`
- LetRec push site (line 1256-1258): Build `EnvScope` from bindings, pass to `LetCleanup::Rec`
- `compute_captures` (line 636-672): Uses `ctx.env.keys()` — update to new API
- `emit_lam` / `emit_thunk`: Create fresh `EmitContext` with `ScopedEnv::new()` — trivial change
- All other `ctx.env.get()` / `ctx.env.insert()` / `ctx.env.contains_key()` calls: API-compatible, no change needed

## Verification

```bash
cargo check -p tidepool-codegen
cargo test -p tidepool-runtime --test text_spliton
cargo test -p tidepool-eval
cargo test --workspace
```

## Boundary

- Do NOT change `in_tail_position` handling (that's the tail-ctx plan)
- Do NOT add new features or optimizations
- Do NOT change any test expectations
- Do NOT rename SsaVal, VarId, or any other existing types
