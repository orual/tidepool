# Polyfill Layer for Unsupported Primops

## Problem

GHC's `showLitChar` (used by `show @Char`, `show @String`) goes through Unicode general category lookup, `Data.Text` internals, pinned `ByteArray#` allocation, and exception handling. Our JIT supports ~50 primops (`Int#`, `Char#`, `Word#` arithmetic/comparison, basic conversions). The fat interface resolver faithfully inlines all of `showLitChar`'s Core, hitting 100+ unsupported primops on live code paths.

## Architecture

### Core Idea: Primop Audit + Polyfill Registry

The resolver already has a resolution chain:

```
idUnfolding → realIdUnfolding → specFallback → preludeSubstitute → fatIface → unresolved
```

We add a **primop audit gate** after fat interface resolution. If the resolved Core's transitive closure contains unsupported primops, check a polyfill registry before accepting it:

```
idUnfolding → realIdUnfolding → specFallback → preludeSubstitute → fatIface → AUDIT → polyfill → unresolved
```

### Three Components

#### 1. Primop Audit (Haskell — Resolve.hs)

A function `auditCorePrimops :: CoreExpr -> Set PrimOp` that walks a resolved CoreExpr and collects all primops used. Compare against a whitelist of supported primops. If any unsupported primops are found, the binding fails audit.

This is a simple recursive walk over the Core AST — no new infrastructure needed.

#### 2. Polyfill Registry (Haskell — Resolve.hs)

A `Map OccName OccName` mapping GHC stdlib binding names to Tidepool.Prelude binding names:

```haskell
polyfillRegistry :: [(String, String)]
polyfillRegistry =
  [ ("showLitChar",  "tpShowLitChar")
  , ("showLitString", "tpShowLitString")
  , ("itos",         "tpItos")          -- in case showInt uses it
  ]
```

When a fat-interface-resolved binding fails primop audit, look up its OccName in the registry. If a polyfill exists (and is available in the local `nameMap` from Tidepool.Prelude), emit an alias binding `NonRec ghcVar (Var polyfillVar)` — exactly like the existing `preludeSubstitute` mechanism.

This generalizes `preludeMethodSubstitutes` (which currently handles `$fOrdList_$ccompare → compareString` etc.) into a single unified system.

#### 3. Polyfill Implementations (Haskell — Tidepool/Prelude.hs)

ASCII-safe implementations of the audited-out functions using only supported primops:

```haskell
-- | Show a Char literal: show 'a' → "'a'"
-- ASCII-only. Uses only Char comparison and Int arithmetic.
tpShowLitChar :: Char -> String -> String
tpShowLitChar c s
  | c == '\\'  = '\\' : '\\' : s
  | c == '\''  = '\\' : '\'' : s
  | c == '\n'  = '\\' : 'n' : s
  | c == '\t'  = '\\' : 't' : s
  | c == '\r'  = '\\' : 'r' : s
  | c >= ' ' && c <= '~' = c : s    -- printable ASCII
  | otherwise  = '\\' : tpShowPosInt (fromEnum c) s  -- numeric escape

-- | Show a String literal: show "hi" → "\"hi\""
tpShowLitString :: String -> String -> String
tpShowLitString []     s = s
tpShowLitString (c:cs) s = tpShowLitChar c (tpShowLitString cs s)
```

The key insight: `showLitChar` has type `Char -> String -> String` (ShowS style). Our polyfill has the same type signature, so it's a drop-in replacement at the Core level.

## Resolution Flow (Detailed)

Current `handleUnfolding` in Resolve.hs line 83:

```haskell
handleUnfolding unfoldingExpr =
  let newBind = NonRec v unfoldingExpr
      newFVs = exprSomeFreeVars (const True) unfoldingExpr
      ...
  in go ...
```

After the change, fat interface resolution becomes:

```haskell
-- In the fatIface case (line 118):
Just fatExpr -> do
  let unsupported = auditCorePrimops fatExpr
  if Set.null unsupported
    then handleUnfolding fatExpr   -- clean: use fat interface Core
    else case lookupPolyfill nameMap vName of
      Just subBind -> ...          -- polyfill available: use it
      Nothing -> handleUnfolding fatExpr  -- no polyfill: use it anyway (may fail at JIT)
```

Note: we still accept the fat interface Core even when audit fails and no polyfill exists. This is intentional — the unsupported primops might be on dead branches (like `raise#` on impossible pattern matches). The JIT's `Raise` handling covers those.

## Supported Primop Whitelist

From `Translate.hs` lines 671-766, our supported primops:

```
Int: Add Sub Mul Negate Eq Ne Lt Le Gt Ge Quot Rem And Or Xor Not Shl Shra Shrl
Word: Add Sub Mul Eq Ne Lt Le Gt Ge Quot Rem And Or Xor Not Shl Shrl
Char: Eq Ne Lt Le Gt Ge
Chr, Ord
Int↔Word: Int2Word Word2Int
Narrow: 8/16/32 Int and Word
Float: Add Sub Mul Div Negate Eq Ne Lt Le Gt Ge
Double: Negate
Conversions: Int2Double Double2Int Int2Float Float2Int Double2Float Float2Double
Addr: IndexCharOffAddr
Exception: Raise (maps to runtime_error)
```

Anything not on this list triggers an audit failure.

## Implementation Plan (Exomonad Worktrees)

### Leaf 1: Polyfill Implementations in Prelude

**Branch:** `main.polyfill.prelude`
**Files:** `haskell/lib/Tidepool/Prelude.hs`
**Task:** Add `tpShowLitChar`, `tpShowLitString`, `tpShowPosInt` to Tidepool.Prelude. Export them. Write them using only supported primops (Char comparisons, Int arithmetic, `fromEnum`). Must match GHC's `ShowS` type: `Char -> String -> String`.

**Verify:**
```bash
# Compiles with the overlay GHC
nix develop --command bash -c "cd haskell && cabal build tidepool-harness"
```

**Boundary:**
- NO imports beyond what Prelude.hs already imports
- NO primops beyond the whitelist above
- Functions must be `{-# INLINE #-}` so GHC exposes their unfoldings
- Use explicit `Int` type signatures on all local bindings (avoid Integer defaulting)

### Leaf 2: Primop Audit + Polyfill Registry in Resolver

**Branch:** `main.polyfill.resolver`
**Files:** `haskell/src/Tidepool/Resolve.hs`
**Task:**

1. Add `auditCorePrimops :: CoreExpr -> Set String` — walks CoreExpr, collects primop OccNames, returns those not in the supported whitelist.

2. Add `polyfillRegistry :: [(String, String)]` — maps GHC stdlib names to Prelude polyfill names. Initial entries:
   - `"showLitChar" → "tpShowLitChar"`
   - `"showLitString" → "tpShowLitString"`
   - `"itos" → "tpItos"` (if needed)

3. Modify the fat interface case (line 118) to audit + polyfill:
   ```haskell
   Just fatExpr -> do
     let badPrimops = auditCorePrimops fatExpr
     if null badPrimops
       then handleUnfolding fatExpr
       else case polyfillSubstitute nameMap vName v of
         Just subBind ->
           let localSet' = extendVarSet localSet v
           in go fatCache nameMap rest visited' localSet' acc (subBind : subAcc) unres
         Nothing -> handleUnfolding fatExpr
   ```

4. Merge `preludeMethodSubstitutes` into `polyfillRegistry` — unify the two substitution mechanisms.

**Verify:**
```bash
nix develop --command bash -c "cd haskell && cabal build tidepool-harness"
# Then test extraction:
result/bin/tidepool-extract --all-closed --include haskell/lib /tmp/test_show_char.hs
# Should see: [polyfill] showLitChar → tpShowLitChar (N unsupported primops in original)
```

**Boundary:**
- The audit walk is SHALLOW — only checks the immediate CoreExpr, not transitive resolution. Transitive deps are audited when they're resolved individually.
- Do NOT change the resolution order for non-fat-interface bindings
- Do NOT remove the existing `preludeSubstitute` mechanism until the polyfill registry subsumes it

**Depends on:** Leaf 1 (polyfill functions must exist in Prelude for `nameMap` lookup to succeed)

### Leaf 3: Regenerate CBOR Fixtures + Test

**Branch:** `main.polyfill.test`
**Files:**
- `haskell/test/Suite.hs` — add `showChar`, `showString` test bindings if not present
- `haskell/test/suite_cbor/` — regenerated fixtures
- `tidepool-eval/tests/haskell_suite.rs` — add eval tests for show @Char, show @String
- `tidepool-runtime/tests/sort_crash.rs` — verify 52/52 JIT tests pass

**Task:**
1. Add test bindings to Suite.hs: `showCharA = show 'a'`, `showString = show "hello"`
2. Regenerate all CBOR fixtures with new tidepool-extract
3. Add eval tests
4. Verify all JIT tests pass (should now be 52/52)

**Verify:**
```bash
cargo test -p tidepool-eval --test haskell_suite
TIDEPOOL_EXTRACT=$(readlink -f result/bin/tidepool-extract) cargo test -p tidepool-runtime --test sort_crash
```

**Depends on:** Leaf 1 + Leaf 2 (need polyfills + audit for show @Char to work)

## Execution Order

```
Leaf 1 (Prelude polyfills)  ──┐
                               ├──→  Leaf 3 (Test + Fixtures)
Leaf 2 (Audit + Registry)  ──┘
```

Leaves 1 and 2 are independent and can be spawned in parallel. Leaf 3 depends on both and runs after they merge.

## Success Criteria

- `show 'a'` → `"'a'"` in both eval and JIT
- `show "hello"` → `"\"hello\""` in both eval and JIT
- `show (Just "hi")` → `"Just \"hi\""` in JIT
- 52/52 JIT tests pass
- 122+ eval tests pass (new show tests added)
- No regressions in existing tests
- Polyfill mechanism is general — adding future polyfills requires only adding a function to Prelude and an entry to the registry
