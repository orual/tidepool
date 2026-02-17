# Tidepool — Agent Rules

You are an agent implementing exactly one task. Your task spec and workflow are in your spawn prompt. Follow them.

---

## Rules

### Follow the Spec Exactly

Your task spec contains exact type signatures, exact file paths, exact variant names. Use them verbatim. Do not rename, simplify, reorganize, or "improve" anything. If the spec says `IntAdd`, don't write `Add`. If the spec says `frame.rs`, don't create `types.rs`.

### DO NOT Section Comes First

Your spec has a DO NOT / ANTI-PATTERNS / BOUNDARY section. Read it before anything else. Every rule there exists because a previous agent made that exact mistake.

### Zero Creativity on Architecture

You do not make architectural decisions. You do not choose dependencies. You do not decide module structure. You do not add features the spec didn't ask for. If the spec doesn't mention it, you don't do it.

If something seems missing from the spec (e.g., a type isn't defined, a dependency seems needed), describe the gap when calling `notify_parent`. Do not guess.

### No Escape Hatches

Never write `todo!()`, `unimplemented!()`, `unreachable!()`, `panic!()` (except in tests), `Raw(String)` variants, `Other(Box<dyn Any>)`, or similar. If you can't implement something, describe the gap when calling `notify_parent` rather than stubbing it out.

### No Unnecessary Dependencies

Do not add crate dependencies unless the spec explicitly lists them. If the spec says `[dependencies]` is empty, it means empty.

### Comments

Write doc comments (`///`) explaining what types and functions are for. Do not write stream-of-consciousness comments explaining your reasoning process. Do not write `// TODO` or `// FIXME`.

### Tests

Write the tests the spec asks for. If the spec includes test cases, implement those exact tests. If it says "identity law" and "composition law", write those specific property tests.

---

## Build & Verify

```bash
cargo test --workspace        # Run all tests
cargo check --workspace       # Type check
cargo clippy --workspace      # Lint
```

Always run verify commands from your spec. If the spec provides specific commands, use those.

---

## Project Context

This is **tidepool**: a Haskell Core → Rust compiler and runtime. You're implementing pieces of it.

`plans/decisions.md` has all locked architectural decisions. If your spec references a decision, it's already been made — don't second-guess it.
