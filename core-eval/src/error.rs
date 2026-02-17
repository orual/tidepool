use crate::value::ThunkId;
use core_repr::{JoinId, PrimOpKind, VarId};

/// Evaluation error.
#[derive(Debug, Clone)]
pub enum EvalError {
    /// Variable not found in environment
    UnboundVar(VarId),
    /// Type mismatch during evaluation
    TypeMismatch { expected: String, got: String },
    /// No matching alternative in case expression
    NoMatchingAlt,
    /// Infinite loop detected (thunk forced itself)
    InfiniteLoop(ThunkId),
    /// Unsupported primop
    UnsupportedPrimOp(PrimOpKind),
    /// Heap exhausted
    HeapExhausted,
    /// Application of non-function value
    NotAFunction,
    /// Jump to unknown join point
    UnboundJoin(JoinId),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::UnboundVar(v) => write!(f, "unbound variable: v_{}", v.0),
            EvalError::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {}, got {}", expected, got)
            }
            EvalError::NoMatchingAlt => write!(f, "no matching case alternative"),
            EvalError::InfiniteLoop(id) => write!(f, "infinite loop: thunk {} forced itself", id.0),
            EvalError::UnsupportedPrimOp(op) => write!(f, "unsupported primop: {:?}", op),
            EvalError::HeapExhausted => write!(f, "heap exhausted"),
            EvalError::NotAFunction => write!(f, "application of non-function value"),
            EvalError::UnboundJoin(id) => write!(f, "jump to unbound join point: j_{}", id.0),
        }
    }
}

impl std::error::Error for EvalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let errs = vec![
            EvalError::UnboundVar(VarId(42)),
            EvalError::TypeMismatch {
                expected: "Int".to_string(),
                got: "Char".to_string(),
            },
            EvalError::NoMatchingAlt,
            EvalError::InfiniteLoop(ThunkId(0)),
            EvalError::UnsupportedPrimOp(PrimOpKind::IntAdd),
            EvalError::HeapExhausted,
            EvalError::NotAFunction,
            EvalError::UnboundJoin(JoinId(7)),
        ];

        for err in errs {
            let s = format!("{}", err);
            assert!(!s.is_empty(), "Display for {:?} should not be empty", err);
        }
    }
}
