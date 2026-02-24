//! Text suite: tests for Data.Text support via ByteArray# primops.
//!
//! Each test loads a pre-compiled CBOR fixture from haskell/test/TextSuite_cbor/,
//! deserializes it, evaluates it, and asserts the expected result.
//!
//! Many of these tests will initially fail because Data.Text relies on
//! ByteArray# primops that are not yet implemented. As we add primop support,
//! tests will progressively pass.
//!
//! Primop tiers needed:
//!   Tier 0 (no ByteArray): text_length, text_null_*, text_empty (GHC optimizes these)
//!   Tier 1 (Word8/Int64 casts): word8ToWord#, wordToWord8#, int64 ops
//!   Tier 2 (ByteArray core): newByteArray#, unsafeFreezeByteArray#, indexWord8Array#, writeWord8Array#
//!   Tier 3 (ByteArray bulk): copyByteArray#, compareByteArrays#, sizeofByteArray#, etc.

use tidepool_eval::{deep_force, env_from_datacon_table, eval, Value, VecHeap};
use tidepool_repr::serial::read::{read_cbor, read_metadata};
use tidepool_repr::{DataConTable, Literal};

static META: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/meta.cbor");

fn table() -> DataConTable {
    read_metadata(META).unwrap()
}

fn eval_fixture(cbor: &[u8]) -> Value {
    let expr = read_cbor(cbor).unwrap();
    let table = table();
    let env = env_from_datacon_table(&table);
    let mut heap = VecHeap::new();
    let val = eval(&expr, &env, &mut heap).unwrap();
    deep_force(val, &mut heap).unwrap()
}

/// Try to evaluate; returns Ok(value) or Err(error message).
fn try_eval_fixture(cbor: &[u8]) -> Result<Value, String> {
    let expr = read_cbor(cbor).unwrap();
    let table = table();
    let env = env_from_datacon_table(&table);
    let mut heap = VecHeap::new();
    match eval(&expr, &env, &mut heap) {
        Ok(val) => match deep_force(val, &mut heap) {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("deep_force error: {}", e)),
        },
        Err(e) => Err(format!("eval error: {}", e)),
    }
}

/// Unwrap GHC boxing: I# x → x, C# x → x, D# x → x, W# x → x.
fn unbox(val: &Value, table: &DataConTable) -> Value {
    if let Value::Con(id, fields) = val {
        if let Some(name) = table.name_of(*id) {
            if matches!(name, "I#" | "C#" | "D#" | "W#") && fields.len() == 1 {
                return fields[0].clone();
            }
        }
    }
    val.clone()
}

fn assert_int(val: &Value, expected: i64, table: &DataConTable) {
    let inner = unbox(val, table);
    match inner {
        Value::Lit(Literal::LitInt(n)) => assert_eq!(n, expected, "expected Int {expected}, got {n}"),
        ref other => panic!("expected Int {expected}, got {other:?}"),
    }
}

fn assert_bool(val: &Value, expected: bool, table: &DataConTable) {
    if let Value::Con(id, fields) = val {
        assert!(fields.is_empty(), "Bool should be nullary, got {fields:?}");
        let name = table.name_of(*id).unwrap();
        let actual = name == "True";
        assert_eq!(actual, expected, "expected Bool {expected}, got {name}");
        return;
    }
    panic!("expected Bool, got {val:?}");
}

fn assert_char(val: &Value, expected: char, table: &DataConTable) {
    let inner = unbox(val, table);
    match inner {
        Value::Lit(Literal::LitChar(c)) => {
            assert_eq!(c, expected, "expected Char {expected:?}, got {c:?}")
        }
        ref other => panic!("expected Char {expected:?}, got {other:?}"),
    }
}

/// Collect a Haskell String (list of Char) into a Rust String.
fn collect_string(val: &Value, table: &DataConTable) -> String {
    let mut result = String::new();
    let mut cur = val;
    loop {
        match cur {
            Value::Con(id, fields) => {
                let name = table.name_of(*id).unwrap();
                if name == "[]" {
                    break;
                } else if name == ":" {
                    assert_eq!(fields.len(), 2, "(:) should have 2 fields");
                    let ch = unbox(&fields[0], table);
                    if let Value::Lit(Literal::LitChar(c)) = ch {
                        result.push(c);
                    } else {
                        panic!("expected Char in string cons, got {ch:?}");
                    }
                    cur = &fields[1];
                } else {
                    panic!("expected [] or (:), got {name}");
                }
            }
            other => panic!("expected string cons cell, got {other:?}"),
        }
    }
    result
}

/// Collect a Haskell list into a Vec<Value>.
fn collect_list(val: &Value, table: &DataConTable) -> Vec<Value> {
    let mut result = Vec::new();
    let mut cur = val;
    loop {
        match cur {
            Value::Con(id, fields) => {
                let name = table.name_of(*id).unwrap();
                if name == "[]" {
                    break;
                } else if name == ":" {
                    assert_eq!(fields.len(), 2);
                    result.push(fields[0].clone());
                    cur = &fields[1];
                } else {
                    panic!("expected [] or (:), got {name}");
                }
            }
            other => panic!("expected list cons cell, got {other:?}"),
        }
    }
    result
}

/// Extract a Text value's content as a Rust String.
/// Data.Text internally is `Text (ByteArray#) offset length`.
fn extract_text(val: &Value, table: &DataConTable) -> String {
    if let Value::Con(id, fields) = val {
        let name = table.name_of(*id).unwrap_or("<unknown>");
        if name == "Text" && fields.len() == 3 {
            let ba = match &fields[0] {
                Value::ByteArray(bs) => bs.lock().unwrap().clone(),
                other => panic!("expected ByteArray in Text, got {other:?}"),
            };
            let off = extract_int_field(&fields[1], table) as usize;
            let len = extract_int_field(&fields[2], table) as usize;
            assert!(
                off + len <= ba.len(),
                "Text slice out of bounds: off={off}, len={len}, ba_len={}",
                ba.len()
            );
            return String::from_utf8(ba[off..off + len].to_vec())
                .expect("Text should be valid UTF-8");
        }
    }
    panic!("expected Text constructor, got {val:?}");
}

/// Extract an i64 from a potentially boxed Int value.
fn extract_int_field(val: &Value, table: &DataConTable) -> i64 {
    match val {
        Value::Lit(Literal::LitInt(n)) => *n,
        Value::Con(id, fields) if fields.len() == 1 => {
            let name = table.name_of(*id).unwrap_or("");
            if name == "I#" {
                if let Value::Lit(Literal::LitInt(n)) = &fields[0] {
                    return *n;
                }
            }
            panic!("expected Int, got Con({name}, {fields:?})");
        }
        other => panic!("expected Int, got {other:?}"),
    }
}

// =============================================================================
// Macro for tests that should currently fail (unsupported primops)
// =============================================================================

/// Test that evaluates a fixture and checks the result.
/// If evaluation fails (e.g. unsupported primop), the test still fails
/// so we can track progress as we implement primops.
macro_rules! text_int {
    ($name:ident, $expected:expr) => {
        #[test]
        fn $name() {
            static CBOR: &[u8] = include_bytes!(concat!(
                "../../haskell/test/TextSuite_cbor/",
                stringify!($name),
                ".cbor"
            ));
            let val = eval_fixture(CBOR);
            let table = table();
            assert_int(&val, $expected, &table);
        }
    };
}

macro_rules! text_bool {
    ($name:ident, $expected:expr) => {
        #[test]
        fn $name() {
            static CBOR: &[u8] = include_bytes!(concat!(
                "../../haskell/test/TextSuite_cbor/",
                stringify!($name),
                ".cbor"
            ));
            let val = eval_fixture(CBOR);
            let table = table();
            assert_bool(&val, $expected, &table);
        }
    };
}

macro_rules! text_char {
    ($name:ident, $expected:expr) => {
        #[test]
        fn $name() {
            static CBOR: &[u8] = include_bytes!(concat!(
                "../../haskell/test/TextSuite_cbor/",
                stringify!($name),
                ".cbor"
            ));
            let val = eval_fixture(CBOR);
            let table = table();
            assert_char(&val, $expected, &table);
        }
    };
}

macro_rules! text_string {
    ($name:ident, $expected:expr) => {
        #[test]
        fn $name() {
            static CBOR: &[u8] = include_bytes!(concat!(
                "../../haskell/test/TextSuite_cbor/",
                stringify!($name),
                ".cbor"
            ));
            let val = eval_fixture(CBOR);
            let table = table();
            let s = collect_string(&val, &table);
            assert_eq!(s, $expected, "expected {:?}, got {:?}", $expected, s);
        }
    };
}

// =============================================================================
// Group 1: Construction (5)
// These all require ByteArray# (Text internal rep)
// =============================================================================

// text_pack: T.pack "hello world" → Text
// Can't assert content until we have ByteArray# support + extract_text
// For now just check it doesn't crash
#[test]
fn text_pack() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_pack.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_pack failed: {:?}", result.err());
}

#[test]
fn text_empty() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_empty.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_empty failed: {:?}", result.err());
}

#[test]
fn text_singleton() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_singleton.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_singleton failed: {:?}", result.err());
}

#[test]
fn text_cons() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_cons.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_cons failed: {:?}", result.err());
}

#[test]
fn text_snoc() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_snoc.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_snoc failed: {:?}", result.err());
}

// =============================================================================
// Group 2: Basic queries (5)
// Some of these GHC may optimize away without hitting ByteArray# primops
// =============================================================================

text_int!(text_length, 5);
text_bool!(text_null_empty, true);
text_bool!(text_null_nonempty, false);
text_char!(text_head, 'a');
text_char!(text_last, 'c');

// =============================================================================
// Group 3: Transformations (5)
// All require ByteArray# manipulation
// =============================================================================

#[test]
fn text_reverse() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_reverse.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_reverse failed: {:?}", result.err());
}

#[test]
fn text_toUpper() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_toUpper.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_toUpper failed: {:?}", result.err());
}

#[test]
fn text_toLower() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_toLower.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_toLower failed: {:?}", result.err());
}

#[test]
fn text_append() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_append.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_append failed: {:?}", result.err());
}

#[test]
fn text_intercalate() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_intercalate.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_intercalate failed: {:?}", result.err());
}

// =============================================================================
// Group 4: Substrings / slicing (5)
// =============================================================================

#[test]
fn text_take() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_take.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_take failed: {:?}", result.err());
}

#[test]
fn text_drop() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_drop.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_drop failed: {:?}", result.err());
}

#[test]
fn text_takeWhile() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_takeWhile.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_takeWhile failed: {:?}", result.err());
}

#[test]
fn text_dropWhile() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_dropWhile.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_dropWhile failed: {:?}", result.err());
}

#[test]
fn text_tail() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_tail.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_tail failed: {:?}", result.err());
}

// =============================================================================
// Group 5: Splitting (5)
// =============================================================================

#[test]
fn text_splitOn() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_splitOn.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_splitOn failed: {:?}", result.err());
}

#[test]
fn text_words() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_words.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_words failed: {:?}", result.err());
}

#[test]
fn text_lines() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_lines.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_lines failed: {:?}", result.err());
}

#[test]
fn text_unwords() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_unwords.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_unwords failed: {:?}", result.err());
}

#[test]
fn text_unlines() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_unlines.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_unlines failed: {:?}", result.err());
}

// =============================================================================
// Group 6: Searching (5)
// =============================================================================

text_bool!(text_isPrefixOf, true);
text_bool!(text_isSuffixOf, true);
text_bool!(text_isInfixOf, true);

#[test]
fn text_find() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_find.cbor");
    let val = eval_fixture(CBOR);
    let table = table();
    // Should be Just 'l'
    if let Value::Con(id, fields) = &val {
        let name = table.name_of(*id).unwrap();
        assert_eq!(name, "Just", "expected Just, got {name}");
        assert_eq!(fields.len(), 1);
        assert_char(&fields[0], 'l', &table);
    } else {
        panic!("expected Maybe, got {val:?}");
    }
}

#[test]
fn text_filter() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_filter.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_filter failed: {:?}", result.err());
}

// =============================================================================
// Group 7: Mapping and folding (5)
// =============================================================================

#[test]
fn text_map() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_map.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_map failed: {:?}", result.err());
}

text_int!(text_foldr, 5);
text_int!(text_foldl, 5);

#[test]
fn text_concatMap() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_concatMap.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_concatMap failed: {:?}", result.err());
}

text_bool!(text_any, true);

// =============================================================================
// Group 8: Conversion (5)
// =============================================================================

text_string!(text_unpack, "hello");
text_int!(text_unpack_length, 5);

#[test]
fn text_show() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_show.cbor");
    let val = eval_fixture(CBOR);
    let table = table();
    let s = collect_string(&val, &table);
    assert_eq!(s, "\"hello\"");
}

text_bool!(text_roundtrip, true);
text_bool!(text_compare, true);

// =============================================================================
// Group 9: Numeric conversions (5)
// =============================================================================

text_int!(text_read_int, 42);

#[test]
fn text_show_int() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_show_int.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_show_int failed: {:?}", result.err());
}

text_bool!(text_length_eq, true);

#[test]
fn text_replicate() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_replicate.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_replicate failed: {:?}", result.err());
}

#[test]
fn text_strip() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_strip.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_strip failed: {:?}", result.err());
}

// =============================================================================
// Group 10: Composition patterns (5)
// =============================================================================

#[test]
fn text_kv() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_kv.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_kv failed: {:?}", result.err());
}

#[test]
fn text_join() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_join.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_join failed: {:?}", result.err());
}

text_bool!(text_nested, true);

#[test]
fn text_replace() {
    static CBOR: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_replace.cbor");
    let result = try_eval_fixture(CBOR);
    assert!(result.is_ok(), "text_replace failed: {:?}", result.err());
}

text_bool!(text_all, true);

#[test]
fn scan_sentinels() {
    use tidepool_repr::frame::CoreFrame;
    let fixtures: &[(&str, &[u8])] = &[
        ("text_null_nonempty", include_bytes!("../../haskell/test/TextSuite_cbor/text_null_nonempty.cbor")),
        ("text_pack", include_bytes!("../../haskell/test/TextSuite_cbor/text_pack.cbor")),
        ("text_singleton", include_bytes!("../../haskell/test/TextSuite_cbor/text_singleton.cbor")),
        ("text_head", include_bytes!("../../haskell/test/TextSuite_cbor/text_head.cbor")),
        ("text_empty", include_bytes!("../../haskell/test/TextSuite_cbor/text_empty.cbor")),
        ("text_length", include_bytes!("../../haskell/test/TextSuite_cbor/text_length.cbor")),
    ];
    // Dump all nodes of text_singleton
    let singleton_cbor: &[u8] = include_bytes!("../../haskell/test/TextSuite_cbor/text_singleton.cbor");
    let expr = read_cbor(singleton_cbor).unwrap();
    for i in 30..80.min(expr.nodes.len()) {
        println!("  [{}] {:?}", i, expr.nodes[i]);
    }
}
