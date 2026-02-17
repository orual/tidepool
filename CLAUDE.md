# Tidepool

Compile freer-simple effect stacks into Cranelift-backed state machines drivable from Rust. Haskell expands, Rust collapses. The language boundary is the hylo boundary.

---

## Rules

All rules from the exomonad project apply here. Additionally:

### Locked Decisions

`plans/decisions.md` is the source of truth for all architectural decisions. Every entry is final. Do not deviate from locked decisions. Do not re-derive them. If you need a decision that isn't there, escalate to the human.

### Plans

`plans/README.md` is the master plan. Phase structure, dependency graph, spec docs per phase — it's all there. Read it before doing anything. Each spec doc in `plans/phase-*/` follows a structured format: Task, Read First, Steps, Verify, Done, Tests, Boundary, Domain Anti-Patterns.

### Research

`plans/research/` contains completed empirical research. These are verified findings, not speculation:
- `01-freer-simple-core-output.md` — GHC 9.12.2 -O2 Core dump analysis (validates the entire premise)
- `02-cranelift-stack-maps-jit.md` — Cranelift 0.116.1 stack map POC (6 tests, all pass)
- `03-ghc-912-api-surface.md` — GHC 9.12 API signatures + freer-simple compatibility

Reference these when implementing. They contain exact API signatures, anti-pattern rules, and empirical data.

---

## Project Structure

```
tidepool/
├── plans/             ← Spec docs, decisions, research (READ FIRST)
│   ├── decisions.md   ← Locked architectural decisions
│   ├── README.md      ← Master plan + dependency graph
│   ├── anti-patterns.md
│   ├── phase-1/       ← core-repr spec
│   ├── phase-2/       ← core-eval, core-heap, core-optimize, core-bridge, core-testing specs
│   ├── phase-3/       ← codegen spec
│   └── research/      ← Completed empirical research
├── flake.nix          ← Dev shell (Rust + GHC 9.12)
└── CLAUDE.md          ← YOU ARE HERE
```

Workspace crates (`core-repr/`, `core-eval/`, etc.) are scaffolded by TL agents during execution. They do not exist yet.

### Build

```bash
nix develop              # Enter dev shell
cargo test --workspace   # Run all tests
cargo check --workspace  # Type check
```

---

## Orchestration Model

This project is built by a tree of agents managed by ExoMonad. Understanding the execution model is mandatory for every TL agent.

### Roles

- **Human (root):** Owns `main`. Makes architectural decisions. Approves phase gates.
- **TL (Claude Opus):** Owns a subtree branch (e.g., `main.core-repr`). Decomposes work into leaf specs, spawns agents, merges their PRs. Never writes implementation code.
- **Leaf (Gemini):** Spawned via `spawn_leaf_subtree`. Owns a leaf branch (e.g., `main.core-repr.scaffold`). Implements one task spec. Files PR. Iterates against Copilot review until clean. Calls `notify_parent` when done.
- **Worker (Gemini):** Spawned via `spawn_workers`. Works in the parent's directory. Does NOT create branches, commit, or file PRs. Writes code, runs verify, calls `notify_parent`. The parent reviews and commits.

### Fire-and-Forget Execution

The TL's workflow is: **decompose -> spec -> spawn -> move on**. The TL does not wait, poll, review intermediate output, or manually re-spec.

**Convergence is leaf + Copilot, not TL:**

1. TL writes spec, spawns leaf, returns immediately
2. Leaf works -> commits -> files PR
3. GitHub poller detects Copilot review comments -> injects into leaf's pane
4. Leaf reads Copilot feedback, fixes, pushes
5. Copilot re-reviews; loop repeats until clean
6. Leaf calls `notify_parent` with `success` -> TL gets `[CHILD COMPLETE]`
7. TL reviews the merged diff (parallel merges may interact), then merges

**`notify_parent` means DONE** — not "I filed a PR." The leaf owns its quality.

### Spawn Tool Selection

All spawn tools take the same structured `AgentSpec` (name, task, read_first, context, steps, verify, done_criteria, boundary). Branch names auto-derived from `spec.name`.

| Tool | Use when | Agent gets |
|------|----------|------------|
| `spawn_leaf_subtree` | Task is well-specified, needs file isolation or parallel safety | Own worktree + branch, files PR, Copilot review loop |
| `spawn_workers` | Well-specified, you want direct control, disjoint file sets | Pane in YOUR directory, no branch/PR, you review via `git diff` |
| `spawn_subtree` | Task needs further decomposition or architectural judgment | Own worktree + branch, full TL tools (10-30x more expensive) |

**Default to `spawn_leaf_subtree`** for implementation tasks. Use `spawn_workers` only when you want the code in your directory (e.g., scaffolding you'll commit yourself). Use `spawn_subtree` only when a sub-TL is genuinely needed.

### Spec Quality (You Only Get One Shot)

Since the TL doesn't iterate on specs, the v1 spec must be production-quality. All `AgentSpec` fields map directly to prompt sections:

| Field | Purpose |
|-------|---------|
| `boundary` | DO NOT rules — known failure modes (rendered FIRST in prompt) |
| `read_first` | Exact files to read before coding |
| `steps` | Numbered concrete actions with code snippets |
| `verify` | Exact shell commands to run |
| `done_criteria` | Measurable checklist for completion |
| `context` | Freeform: code snippets, type signatures, examples |

**Anti-patterns / boundary section is mandatory and comes first.** Known Gemini failure modes:

| Failure Mode | Rule |
|---|---|
| Adds unnecessary dependencies | "ZERO external deps. Do NOT add serde/tokio/etc." |
| Invents escape hatches | "No `todo!()`, `Raw(String)`, `Other(Box<dyn Any>)`" |
| Writes thinking-out-loud comments | "Doc comments only. No stream-of-consciousness." |
| Renames types/variants | "Use EXACT type signatures below." |
| Makes architectural decisions | "Do not change module structure." |
| Overengineers | "This is N lines in M files, not a new module." |

Specs are self-contained. The leaf has no context from previous attempts. Include complete code snippets and full file paths.

### Escalation, Not Iteration

If a leaf fails after 3+ Copilot rounds, it calls `notify_parent` with `failure`. The TL then: re-decomposes (smaller leaves), tries a different approach, or escalates to the human. The TL never manually fixes a leaf's code.

### Branch Hierarchy

```
main                              [human]
├── main.core-repr                [TL - Claude]
│   ├── main.core-repr.scaffold   [leaf - Gemini]
│   ├── main.core-repr.serial     [leaf - Gemini]
│   └── main.core-repr.pretty     [leaf - Gemini]
├── main.core-eval                [TL - Claude]
│   └── ...
```

PRs target parent branch (not main). Merged via recursive fold up the tree.

---

## Key Decisions Reference

See `plans/decisions.md` for the full table. Critical ones for daily work:

- **CoreFrame variants:** Var, Lit, App, Lam, LetNonRec, LetRec, Case, Con, Join, Jump, PrimOp
- **No type variants** — types stripped at serialization in Haskell
- **RecursiveTree\<CoreFrame\>** as CoreExpr type alias
- **CBOR** via serialise (Haskell) / ciborium (Rust)
- **Cast/Tick/Type erasure** happens in Haskell serializer, NOT in Rust
- **HeapObject:** manual memory layout (raw byte buffers + unsafe accessors), NOT a Rust enum
- **GC:** Copying collector, custom RBP frame walker, split gc-trace/gc-compact
- **freer-simple continuations:** Leaf/Node tree (type-aligned sequence), NOT single closures
- **Union tags:** unboxed Word# constants (0##, 1##, ...) indexing the effect type list
