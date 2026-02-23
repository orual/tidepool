# Tidepool Plans

## Active: Polyfill Layer for Unsupported Primops

**Goal:** Principled mechanism for substituting GHC stdlib implementations that use unsupported primops with lightweight polyfills that only use our supported primop set.

**Spec:** [polyfill-layer.md](./polyfill-layer.md)

**Status:** Ready to implement

**Context:** Fat interface resolution (`mi_extra_decls`) now resolves loop-breakers like `$fShowCallStack_itos'`, bringing us from 0/52 → 48/52 JIT tests. The remaining 4 failures (`show @Char`, `show @String`, `show @(Maybe String)`) fail because `showLitChar` pulls in the entire Unicode/Text/ByteArray universe — hundreds of primops we don't support. A polyfill layer lets us intercept these and provide ASCII-safe implementations.
