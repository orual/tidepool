# Tidepool

Compile Haskell effect stacks into Cranelift-backed state machines drivable from Rust.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

## What is Tidepool?

Tidepool compiles [freer-simple](https://hackage.haskell.org/package/freer-simple) effect stacks from Haskell into native code via Cranelift JIT, producing effect machines that can be driven step-by-step from Rust. Write your business logic as a pure Haskell effect program, compile it once, then run it with Rust-side effect handlers that provide IO, state, networking, or anything else.

Haskell expands (describes what to do). Rust collapses (does it). The language boundary is the hylo boundary.

## Quick Start

```bash
# Enter the dev shell (provides Rust + GHC 9.12)
nix develop

# Run the number guessing game (JIT-compiled)
cargo run -p tidepool-guess

# Run the interactive REPL
cargo run -p tidepool-tide

# Run the MCP server example
cargo run -p mcp-server-example
```

## Architecture

```
tidepool/                   Facade crate (re-exports public API)
tidepool-repr/              Core IR: CoreExpr, DataConTable, CBOR serialization
tidepool-eval/              Tree-walking interpreter: Value, Env, lazy evaluation
tidepool-heap/              Manual heap + copying GC for JIT runtime
tidepool-optimize/          Optimization passes: beta reduction, DCE, inlining, case reduction
tidepool-bridge/            FromCore/ToCore traits for Rust <-> Core value conversion
tidepool-bridge-derive/     Proc-macro: #[derive(FromCore)]
tidepool-macro/             Proc-macro: haskell_inline! { ... }
tidepool-effect/            Effect handling: EffectHandler trait, HList dispatch
tidepool-codegen/           Cranelift JIT compiler + effect machine
tidepool-runtime/           High-level API: compile_haskell, compile_and_run, caching
tidepool-mcp/               MCP server library (generic over effect handlers)
```

## How It Works

1. **Write Haskell** using `freer-simple` effects (e.g. `emit "hello" >> awaitInt`)
2. **Extract GHC Core** via a Haskell plugin (`tidepool-extract`) that serializes to CBOR
3. **Load in Rust** as `CoreExpr` + `DataConTable` (the IR)
4. **Optimize** with configurable passes (beta reduction, inlining, dead code elimination)
5. **Compile to native** via Cranelift, producing a `JitEffectMachine`
6. **Run with handlers** — the machine yields effect requests; Rust handlers respond

```rust
use tidepool_macro::haskell_inline;
use tidepool_codegen::jit_machine::JitEffectMachine;

// Compile Haskell at build time
let (expr, table) = haskell_inline! {
    target = "greet",
    include = "haskell",
    r#"
greet :: Eff '[Console] ()
greet = emit "Hello from Haskell!"
    "#
};

// JIT compile and run with Rust handlers
let mut vm = JitEffectMachine::compile(&expr, &table, 1 << 20)?;
vm.run(&table, &mut handlers, &())?;
```

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](http://opensource.org/licenses/MIT) at your option.
