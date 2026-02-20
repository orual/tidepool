//! Tree-walking interpreter for Tidepool Core expressions.
//!
//! Provides `Value`, environment management, thunk allocation, and a lazy
//! evaluator that reduces `CoreExpr` to `Value`.

pub mod env;
pub mod error;
pub mod eval;
pub mod heap;
pub mod pass;
pub mod value;

pub use env::*;
pub use error::*;
pub use eval::*;
pub use heap::*;
pub use pass::*;
pub use value::*;
