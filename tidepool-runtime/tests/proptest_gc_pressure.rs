use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use tidepool_optimize::pipeline::optimize;
use tidepool_repr::frame::CoreFrame;
use tidepool_repr::types::{DataConId, Literal, VarId};
use tidepool_repr::TreeBuilder;
use tidepool_testing::gen::arb_core_expr;
use tidepool_testing::proptest::check_jit_vs_eval;

#[test]
fn allocation_heavy_tiny_nursery() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut runner = TestRunner::new(Config {
                cases: 50,
                ..Config::default()
            });
            runner
                .run(
                    &arb_core_expr().prop_filter("at least 3 Con nodes", |expr| {
                        expr.nodes
                            .iter()
                            .filter(|n| matches!(n, CoreFrame::Con { .. }))
                            .count()
                            >= 3
                    }),
                    |expr| check_jit_vs_eval(expr, 2 * 1024),
                )
                .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn nested_con_chain() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut runner = TestRunner::new(Config {
                cases: 50,
                ..Config::default()
            });
            runner
                .run(&(10..40usize), |depth| {
                    let mut bld = TreeBuilder::new();
                    let mut current = bld.push(CoreFrame::Lit(Literal::LitInt(42)));
                    for _ in 0..depth {
                        current = bld.push(CoreFrame::Con {
                            tag: DataConId(1),
                            fields: vec![current],
                        });
                    }
                    let expr = bld.build();
                    check_jit_vs_eval(expr, 4 * 1024)
                })
                .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn jit_1kb_nursery_agrees() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut runner = TestRunner::new(Config {
                cases: 50,
                ..Config::default()
            });
            runner
                .run(&arb_core_expr(), |expr| check_jit_vs_eval(expr, 1024))
                .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn optimize_then_tiny_nursery() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut runner = TestRunner::new(Config {
                cases: 50,
                ..Config::default()
            });
            runner
                .run(&arb_core_expr(), |mut expr| {
                    optimize(&mut expr).unwrap();
                    check_jit_vs_eval(expr, 2 * 1024)
                })
                .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn nested_pair_chain() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut runner = TestRunner::new(Config {
                cases: 50,
                ..Config::default()
            });
            runner
                .run(&(3..15usize), |n| {
                    let mut bld = TreeBuilder::new();
                    // Build bottom-up: start with the body
                    let mut body = bld.push(CoreFrame::Var(VarId(n as u64)));

                    // We want: let v0 = (0, 1) in let v1 = (1, v0) in ... let vN = (N, v_{N-1}) in vN
                    // To build this bottom-up, we need to build LetNonRec for vN first, then v_{N-1}, etc.
                    for i in (0..=n).rev() {
                        let rhs = if i == 0 {
                            let l0 = bld.push(CoreFrame::Lit(Literal::LitInt(0)));
                            let l1 = bld.push(CoreFrame::Lit(Literal::LitInt(1)));
                            bld.push(CoreFrame::Con {
                                tag: DataConId(4),
                                fields: vec![l0, l1],
                            })
                        } else {
                            let li = bld.push(CoreFrame::Lit(Literal::LitInt(i as i64)));
                            let prev = bld.push(CoreFrame::Var(VarId((i - 1) as u64)));
                            bld.push(CoreFrame::Con {
                                tag: DataConId(4),
                                fields: vec![li, prev],
                            })
                        };
                        body = bld.push(CoreFrame::LetNonRec {
                            binder: VarId(i as u64),
                            rhs,
                            body,
                        });
                    }

                    let expr = bld.build();
                    check_jit_vs_eval(expr, 2 * 1024)
                })
                .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
