//! Option 4: Optimization preserves evaluation semantics.
//!
//! For every generated expression, `eval(expr) == eval(optimize(expr))`.
//! Catches optimizer soundness bugs where a transformation changes behavior.

use proptest::test_runner::{Config, TestRunner};
use std::cell::Cell;
use tidepool_eval::{env::Env, eval::eval, heap::VecHeap};
use tidepool_optimize::optimize;
use tidepool_testing::compare;
use tidepool_testing::gen::arb_ground_expr;

#[test]
fn optimization_preserves_semantics() {
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut runner = TestRunner::new(Config {
                cases: 200,
                ..Config::default()
            });
            let compared = Cell::new(0u64);
            let both_error = Cell::new(0u64);
            let deep_force_fail = Cell::new(0u64);
            let eval_only_error = Cell::new(0u64);

            runner
                .run(&arb_ground_expr(), |expr| {
                    // Evaluate original
                    let mut heap1 = VecHeap::new();
                    let before = eval(&expr, &Env::new(), &mut heap1);

                    // Optimize
                    let mut optimized = expr.clone();
                    let _ = optimize(&mut optimized); // ignore stats

                    // Evaluate optimized
                    let mut heap2 = VecHeap::new();
                    let after = eval(&optimized, &Env::new(), &mut heap2);

                    match (before, after) {
                        (Ok(v1), Ok(v2)) => {
                            let f1 = tidepool_eval::eval::deep_force(v1, &mut heap1);
                            let f2 = tidepool_eval::eval::deep_force(v2, &mut heap2);
                            match (f1, f2) {
                                (Ok(fv1), Ok(fv2)) => {
                                    compare::assert_values_eq(&fv1, &fv2);
                                    compared.set(compared.get() + 1);
                                }
                                (Err(_), Err(_)) => { deep_force_fail.set(deep_force_fail.get() + 1); }
                                (Ok(v), Err(e)) => {
                                    panic!("optimize broke deep_force: before Ok({}) after Err({:?})", v, e)
                                }
                                (Err(_), Ok(_)) => { deep_force_fail.set(deep_force_fail.get() + 1); }
                            }
                        }
                        (Err(_), Err(_)) => { both_error.set(both_error.get() + 1); }
                        (Ok(v1), Err(e)) => {
                            // Optimizer may make lazy errors strict (case-of-known-constructor
                            // inlines previously-thunked error paths). Verify: does deep_force
                            // of the original also error?
                            let forced = tidepool_eval::eval::deep_force(v1, &mut heap1);
                            match forced {
                                Err(_) => {
                                    // Both error on deep_force — acceptable strictness change
                                    both_error.set(both_error.get() + 1);
                                }
                                Ok(_) => {
                                    panic!(
                                        "optimize broke eval: original deep_forces Ok but optimized errors: {:?}",
                                        e
                                    );
                                }
                            }
                        }
                        (Err(_), Ok(_)) => { eval_only_error.set(eval_only_error.get() + 1); }
                    }
                    Ok(())
                })
                .unwrap();

            let compared = compared.get();
            let both_error = both_error.get();
            let eval_only_error = eval_only_error.get();
            let deep_force_fail = deep_force_fail.get();
            eprintln!(
                "\nOptimize: compared={compared}, both_error={both_error}, \
                 eval_only_error={eval_only_error}, deep_force_fail={deep_force_fail}"
            );
            assert!(
                compared >= 50,
                "Only {compared} of 200 cases reached value comparison"
            );
        })
        .unwrap();
    handle.join().unwrap();
}
