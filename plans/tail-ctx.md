# Plan: TailCtx through hylomorphism

Replace the ambient `in_tail_position: bool` mutable field on `EmitContext` with an explicit `TailCtx` parameter threaded through the hylomorphism. This makes it structurally impossible for sub-expressions to accidentally inherit tail position from their parent.

## Motivation

The LetBoundary bug: `in_tail_position = true` leaked from a Phase 3a lambda body through the hylomorphism's `collapse_frame` into a Case scrutinee's LetRec body, causing it to be compiled as a tail call. The fix was a manual save/restore in `collapse_frame`. With an explicit parameter, sub-expressions receive `NonTail` by construction — no save/restore needed, no leak possible.

## New type (in `emit/mod.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailCtx {
    /// Expression result is the function's return value. App may use TCO.
    Tail,
    /// Expression result is consumed by a parent (Case scrutinee, App arg, etc.)
    NonTail,
}

impl TailCtx {
    pub fn is_tail(self) -> bool { matches!(self, TailCtx::Tail) }
}
```

## Design

The key insight: the hylomorphism (`emit_subtree`) always evaluates **mapped children**, and mapped children are never in tail position (the parent frame still has work to do). So `collapse_frame` should always receive `NonTail` for its children — this is the invariant we're encoding.

The work stack in `emit_node` is the only place where tail position matters: the **final** expression evaluated (the Let body, the LetRec body) inherits the caller's tail context. RHS evaluations are always `NonTail`.

## Changes by file

### `emit/mod.rs`
- Add `TailCtx` enum
- Remove `in_tail_position: bool` from `EmitContext`
- Remove from `EmitContext::new`

### `emit/expr.rs`

#### `collapse_frame` — remove the LetBoundary save/restore hack
```rust
// Before (manual save/restore):
EmitFrame::LetBoundary(idx) => {
    let saved_tail = ctx.in_tail_position;
    ctx.in_tail_position = false;
    let result = ctx.emit_node(sess, builder, idx);
    ctx.in_tail_position = saved_tail;
    result
}

// After (NonTail by construction):
EmitFrame::LetBoundary(idx) => {
    ctx.emit_node(sess, builder, idx, TailCtx::NonTail)
}
```

#### `emit_subtree` — no change needed
The hylomorphism itself doesn't care about TailCtx. `collapse_frame` receives `TailCtx::NonTail` implicitly because all its children were mapped (i.e., not in tail position).

#### `emit_node` — thread `TailCtx` as parameter
```rust
pub fn emit_node(
    &mut self,
    sess: &mut EmitSession,
    builder: &mut FunctionBuilder,
    root_idx: usize,
    tail: TailCtx,          // NEW parameter
) -> Result<SsaVal, EmitError> {
```

#### Work stack changes
- Remove `EmitWork::SetTailPosition(bool)` variant entirely
- Replace `self.in_tail_position` reads with a local `tail_ctx: TailCtx` variable
- LetNonRec: body gets caller's `tail_ctx`, RHS gets `NonTail`
- LetRec: body gets caller's `tail_ctx` (via `LetRecFinish`), all RHS get `NonTail`
- The inner loop's App tail-call check: `if tail_ctx.is_tail() && matches!(...)`

Concretely, the work stack items that carry tail context:

```rust
enum EmitWork {
    Eval(usize),
    EvalTail(usize, TailCtx),   // replaces Eval + SetTailPosition pairs
    Bind(VarId),
    LetRecPostSimple { binder: VarId, state_idx: usize },
    LetRecFinish { body: usize, state_idx: usize, tail: TailCtx },
    LetCleanupMark(LetCleanup),
    // SetTailPosition REMOVED
}
```

The inner `loop` in `emit_node` tracks a local `current_tail: TailCtx`:
- Starts as the `tail` parameter
- `EvalTail(idx, t)` sets `current_tail = t` then processes `idx`
- `LetRecFinish` carries the tail context for the body

Alternative (simpler): keep a single local `mut tail_ctx: TailCtx` variable and update it from work items. This is equivalent to the old `SetTailPosition` but scoped to the function, not the struct.

```rust
// LetNonRec push (LIFO):
work.push(LetCleanupMark(...));
work.push(EvalWithTail(body, caller_tail));  // body inherits
work.push(Bind(binder));
work.push(EvalWithTail(rhs, TailCtx::NonTail));  // RHS never tail
```

#### `emit_tail_app` — no change
Already saves/restores for sub-expressions. With the new design, it receives `TailCtx::Tail` from the caller and evaluates fun/arg with `TailCtx::NonTail` internally (via `emit_subtree`, which always passes NonTail through the hylomorphism).

#### `emit_lam` / `emit_thunk` / Phase 3a lambdas
- `emit_lam`: calls `inner_emit.emit_node(sess, builder, body_idx, TailCtx::Tail)` — lambda body IS tail
- `emit_thunk`: calls `inner_emit.emit_node(sess, builder, body_idx, TailCtx::NonTail)` — thunk returns to heap_force
- Phase 3a: calls `inner_emit.emit_node(sess, builder, body_idx, TailCtx::Tail)` — same as emit_lam

### `emit/case.rs`
- `emit_case`: calls `ctx.emit_node(sess, builder, alt.body)` — needs tail context.
  The case result IS in tail position if the case itself is. But `emit_case` is called from `collapse_frame` which is always `NonTail`, OR from `emit_node` which knows its tail context.

  **Decision**: `emit_case` receives a `TailCtx` parameter and passes it to alt body emission. When called from `collapse_frame` (mapped child), it gets `NonTail`. When called from `emit_node`... actually, case is always handled via the hylomorphism (`emit_subtree`), never directly from `emit_node`. So `emit_case` always gets `NonTail` from collapse_frame.

  Wait — Case alt bodies are NOT mapped children. Looking at `EmitFrame::Case` mapping (expr.rs lines 106-114): only the scrutinee is mapped. Alt bodies are passed through as indices and emitted directly inside `emit_case` → `emit_data_dispatch` → `ctx.emit_node(sess, builder, alt.body)`. So these `emit_node` calls need... `NonTail` too, since the Case frame itself was a mapped child.

  Actually, if a Case is the last expression in a function (tail position), its alt bodies ARE in tail position. But since Case arrives via the hylomorphism which is always `NonTail`, this is a missed optimization, not a correctness bug. We can pass `NonTail` for now and add tail-through-case as a follow-up optimization.

  **For this plan**: `emit_case` and helpers pass `TailCtx::NonTail` to all `emit_node` calls. This is conservative and correct.

### `emit/join.rs`
- `emit_join`: calls `ctx.emit_node` for body and rhs. Pass `TailCtx::NonTail` (join points are always inside larger expressions).

## Verification

```bash
cargo check -p tidepool-codegen
cargo test -p tidepool-runtime --test text_spliton
cargo test -p tidepool-eval
cargo test --workspace
```

## Boundary

- Do NOT change env save/restore patterns (that's the scoped-env plan)
- Do NOT add tail-through-case optimization yet — just pass NonTail conservatively
- Do NOT change test expectations
- Do NOT rename existing types besides removing `in_tail_position` field
- The `SetTailPosition` work item is REMOVED, not repurposed
