# tidepool — Haskell-in-Rust via Cranelift

Compile freer-simple effect stacks into Cranelift-backed state machines drivable from Rust. Haskell expands, Rust collapses. The language boundary is the hylo boundary.

## Dependency Graph

```
phase-1/core-repr
    ↓
phase-2/ (parallel where deps allow)
  ├── core-eval        (needs CoreFrame)
  ├── core-heap         (needs HeapObject from core-eval scaffold)
  ├── core-optimize     (needs working evaluator from core-eval)
  ├── core-bridge       (needs CoreFrame + Value from core-eval scaffold)
  └── core-testing      (generators early; oracle + bench need eval)
    ↓
phase-3/codegen         (needs eval + heap + optimize)
```

**Sequencing within phase 2:** core-eval scaffolds first (defines HeapObject, Heap trait, Value). core-heap and core-bridge can start after that scaffold lands. core-optimize needs the full evaluator as its test oracle. In practice, core-eval runs first, then the others fan out.

## Orchestration Model

### Primitives

| Primitive | Tool | Isolation | Use When |
|-----------|------|-----------|----------|
| **Leaf subtree** | `spawn_leaf_subtree` | Own worktree + branch + Copilot review | **Default.** Any well-specified implementation task. |
| **Worker** | `spawn_workers` | Same dir as parent TL | Single-agent scaffolding you'll commit yourself, OR multiple agents with provably zero file overlap. |
| **Claude subtree** | `spawn_subtree` | Own worktree + branch + full TL tools | Coordination nodes that need further decomposition or judgment. 10-30x more expensive. |

### Default: Leaf Subtrees, Not Workers

`spawn_leaf_subtree` is the default for implementation tasks:

- Each leaf gets its own worktree, branch, and Copilot review loop — free quality gating.
- File isolation prevents conflicts when agents run in parallel. Almost every crate has shared coordination files (lib.rs, Cargo.toml, mod declarations) — "disjoint files" is harder to achieve than it looks.
- The branch + PR overhead is handled automatically by tooling.
- TL merges each leaf's PR sequentially after it passes Copilot review.

Use `spawn_workers` only when:
- **Single agent** doing scaffolding you'll commit yourself (e.g., Wave 1 skeleton).
- **Multiple agents** with provably zero file overlap — not even lib.rs or Cargo.toml. If you have to think about whether files overlap, use leaf subtrees.

**Disjointness litmus test:** Can you list every file each agent touches, and the lists don't intersect at all? If any agent adds `mod` declarations, deps, or re-exports to a shared file, they are NOT disjoint.

### TL Lifecycle

Each TL spec doc describes waves of work. A TL's job:

1. Read its spec doc
2. Scaffold wave: spawn a worker to write types/traits/signatures. Review output. Commit. This is the one place workers shine — single agent, you commit directly.
3. Implementation waves: spawn parallel leaf subtrees for independent tasks. Each leaf gets a focused spec with exact code snippets, file paths, and test commands. Each files its own PR and iterates with Copilot.
4. Merge: as each leaf calls `notify_parent`, review its PR diff and merge. Watch for interactions between parallel merges.
5. Verify: `cargo test --workspace` after merging. If red, diagnose and re-spec.
6. When all waves complete, file PR against parent branch.

### Failure Protocol

Leaf failure (Copilot review stuck, build breaks):
1. Leaf calls `notify_parent` with `failure` after 3+ rounds
2. TL reads the PR diff and failure message
3. TL writes a sharper spec addressing the specific failure
4. TL spawns a fresh leaf with the improved spec
5. After 3 failures on the same task: split it smaller or escalate to human

### Quality Gates

Some scaffolds need TL review before downstream work starts (marked "gate" in specs). At gates, the TL:
- Reads the scaffold output (worker diff or leaf PR)
- Verifies type signatures match the locked decisions
- Runs `cargo test`
- Commits (worker) or merges PR (leaf) if clean, re-specs if not
- Only then spawns the next wave

## Tree Shape

```
main [Human]
│
├── core-repr [Claude TL, depth 1]
│     worker: scaffold (gate)
│     leaves: frame-utils, types-datacon, serial, pretty
│     subtree: haskell-harness [Claude, depth 2]
│       └── leaves: ghc-api-harness, core-serializer, wiring
│
├── core-eval [Claude TL, depth 1]
│     worker: scaffold (gate)
│     leaves: eval-strict, eval-case, thunks, join-points
│
├── core-heap [Claude TL, depth 1]
│     worker: scaffold+arena (gate)
│     leaves: gc-trace, gc-compact
│
├── core-optimize [Claude TL, depth 1]
│     worker: scaffold+occ+beta+case-reduce (gate)
│     leaves: inline (coalg+alg), dce, partial (subst-hylo, reduce-hylo)
│
├── core-bridge [Claude TL, depth 1]
│     worker: scaffold (gate)
│     leaves: traits, derive-parse, derive-codegen, haskell-macro
│
├── core-testing [Claude TL, depth 1]
│     worker: scaffold (gate)
│     leaves: generators, differential, bench
│
└── codegen [Claude TL, depth 2 — child of core-eval branch]
      worker: scaffold (gate)
      leaves: codegen-expr, case-and-join, gc-integration, yield
```

## Statistics

```
Claude subtrees:  8  (core-repr, haskell-harness, core-eval, core-heap,
                      core-optimize, core-bridge, core-testing, codegen)
Gemini leaves:   ~30  (most implementation work, each with Copilot review)
Gemini workers:   ~8  (scaffold gates only)
Max depth:        2  (main → core-repr → haskell-harness, main → core-eval → codegen)
Max parallelism: ~6-8  concurrent leaves across active TLs
```

## Docs

| File | Contents |
|------|----------|
| `decisions.md` | Locked design decisions (CoreFrame, HeapObject, GHC pipeline) |
| `phase-1/core-repr.md` | CoreFrame types, CBOR serial, pretty printer, Haskell harness |
| `phase-2/core-eval.md` | Tree-walking evaluator (strict, case, lazy, thunks, join) |
| `phase-2/core-heap.md` | Arena allocator + copying GC |
| `phase-2/core-optimize.md` | Optimization passes + first-order partial eval |
| `phase-2/core-bridge.md` | FromCore/ToCore traits, derive macros, haskell! macro |
| `phase-2/core-testing.md` | Proptest generators, GHC differential oracle, benchmarks |
| `phase-3/codegen.md` | Cranelift backend + EffectMachine |
| `anti-patterns.md` | Shared base anti-patterns for all workers |
| `research/01-freer-simple-core-output.md` | **COMPLETE:** actual -O2 Core structure from freer-simple (GHC 9.12.2) |
| `research/02-cranelift-stack-maps-jit.md` | **COMPLETE:** Cranelift stack map semantics + JIT pipeline (cranelift 0.116.1) |
| `research/03-ghc-912-api-surface.md` | **COMPLETE:** GHC 9.12 API for harness + freer-simple compatibility |
