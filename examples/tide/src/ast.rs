use core_bridge::{BridgeError, ToCore};
use core_eval::value::Value;
use core_repr::DataConTable;

/// Tide expression AST — mirrors Haskell's TExpr data type.
#[derive(Debug, Clone, PartialEq)]
pub enum TExpr {
    TInt(i64),
    TStr(String),
    TBool(bool),
    TVar(String),
    TList(Vec<TExpr>),
    TApp(Box<TExpr>, Vec<TExpr>),
    TBuiltin(i64, Vec<TExpr>),
    TLet(String, Box<TExpr>, Box<TExpr>),
    TLam(Vec<String>, Box<TExpr>),
    TIf(Box<TExpr>, Box<TExpr>, Box<TExpr>),
    TBinOp(i64, Box<TExpr>, Box<TExpr>),
}

impl ToCore for TExpr {
    fn to_value(&self, table: &DataConTable) -> Result<Value, BridgeError> {
        match self {
            TExpr::TInt(n) => {
                let con = lookup(table, "TInt")?;
                Ok(Value::Con(con, vec![n.to_value(table)?]))
            }
            TExpr::TStr(s) => {
                let con = lookup(table, "TStr")?;
                Ok(Value::Con(con, vec![s.to_value(table)?]))
            }
            TExpr::TBool(b) => {
                let con = lookup(table, "TBool")?;
                Ok(Value::Con(con, vec![b.to_value(table)?]))
            }
            TExpr::TVar(s) => {
                let con = lookup(table, "TVar")?;
                Ok(Value::Con(con, vec![s.to_value(table)?]))
            }
            TExpr::TList(es) => {
                let con = lookup(table, "TList")?;
                Ok(Value::Con(con, vec![es.to_value(table)?]))
            }
            TExpr::TApp(f, args) => {
                let con = lookup(table, "TApp")?;
                Ok(Value::Con(con, vec![
                    (**f).to_value(table)?,
                    args.to_value(table)?,
                ]))
            }
            TExpr::TBuiltin(id, args) => {
                let con = lookup(table, "TBuiltin")?;
                Ok(Value::Con(con, vec![
                    id.to_value(table)?,
                    args.to_value(table)?,
                ]))
            }
            TExpr::TLet(name, val, body) => {
                let con = lookup(table, "TLet")?;
                Ok(Value::Con(con, vec![
                    name.to_value(table)?,
                    (**val).to_value(table)?,
                    (**body).to_value(table)?,
                ]))
            }
            TExpr::TLam(params, body) => {
                let con = lookup(table, "TLam")?;
                Ok(Value::Con(con, vec![
                    params.to_value(table)?,
                    (**body).to_value(table)?,
                ]))
            }
            TExpr::TIf(cond, t, e) => {
                let con = lookup(table, "TIf")?;
                Ok(Value::Con(con, vec![
                    (**cond).to_value(table)?,
                    (**t).to_value(table)?,
                    (**e).to_value(table)?,
                ]))
            }
            TExpr::TBinOp(op, l, r) => {
                let con = lookup(table, "TBinOp")?;
                Ok(Value::Con(con, vec![
                    op.to_value(table)?,
                    (**l).to_value(table)?,
                    (**r).to_value(table)?,
                ]))
            }
        }
    }
}

fn lookup(table: &DataConTable, name: &str) -> Result<core_repr::DataConId, BridgeError> {
    table
        .get_by_name(name)
        .ok_or_else(|| BridgeError::UnknownDataConName(name.into()))
}
