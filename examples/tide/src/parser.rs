use crate::ast::TExpr;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "tide.pest"]
pub struct TideParser;

pub fn parse(input: &str) -> Result<TExpr, String> {
    let pairs = TideParser::parse(Rule::program, input)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    let pair = pairs.into_iter().next().unwrap();
    parse_expr(pair.into_inner().next().unwrap())
}

fn parse_expr(pair: Pair<Rule>) -> Result<TExpr, String> {
    match pair.as_rule() {
        Rule::let_expr => parse_let(pair),
        Rule::if_expr => parse_if(pair),
        Rule::lambda_expr => parse_lambda(pair),
        Rule::comparison => parse_comparison(pair),
        Rule::expr => parse_expr(pair.into_inner().next().unwrap()),
        _ => Err(format!("Unexpected rule in parse_expr: {:?}", pair.as_rule())),
    }
}

fn parse_let(pair: Pair<Rule>) -> Result<TExpr, String> {
    let mut inner = pair.into_inner();
    let ident = inner.next().unwrap().as_str().to_string();
    let val = parse_expr(inner.next().unwrap())?;
    let body = parse_expr(inner.next().unwrap())?;
    Ok(TExpr::TLet(ident, Box::new(val), Box::new(body)))
}

fn parse_if(pair: Pair<Rule>) -> Result<TExpr, String> {
    let mut inner = pair.into_inner();
    let cond = parse_expr(inner.next().unwrap())?;
    let t = parse_expr(inner.next().unwrap())?;
    let e = parse_expr(inner.next().unwrap())?;
    Ok(TExpr::TIf(Box::new(cond), Box::new(t), Box::new(e)))
}

fn parse_lambda(pair: Pair<Rule>) -> Result<TExpr, String> {
    let mut inner = pair.into_inner().collect::<Vec<_>>();
    let body_pair = inner.pop().unwrap();
    let params = inner.into_iter().map(|p| p.as_str().to_string()).collect();
    let body = parse_expr(body_pair)?;
    Ok(TExpr::TLam(params, Box::new(body)))
}

fn parse_comparison(pair: Pair<Rule>) -> Result<TExpr, String> {
    parse_binary_op(pair, parse_concat, Rule::comp_op, map_comp_op)
}

fn parse_concat(pair: Pair<Rule>) -> Result<TExpr, String> {
    parse_binary_op(pair, parse_addition, Rule::concat_op, |_| 10)
}

fn parse_addition(pair: Pair<Rule>) -> Result<TExpr, String> {
    parse_binary_op(pair, parse_multiplication, Rule::add_op, |s| {
        if s == "+" { 0 } else { 1 }
    })
}

fn parse_multiplication(pair: Pair<Rule>) -> Result<TExpr, String> {
    parse_binary_op(pair, parse_unary, Rule::mul_op, |s| {
        if s == "*" { 2 } else { 3 }
    })
}

fn parse_binary_op<F, M>(
    pair: Pair<Rule>,
    next: F,
    op_rule: Rule,
    mapper: M,
) -> Result<TExpr, String>
where
    F: Fn(Pair<Rule>) -> Result<TExpr, String>,
    M: Fn(&str) -> i64,
{
    let mut inner = pair.into_inner();
    let first = inner.next().ok_or_else(|| "Missing left operand".to_string())?;
    let mut left = next(first)?;

    while let Some(op_pair) = inner.next() {
        let op_str = if op_pair.as_rule() == op_rule {
            op_pair.as_str()
        } else {
            // Some rules like comp_op have nested ops
            op_pair.into_inner().next().unwrap().as_str()
        };
        let op_id = mapper(op_str);
        let right = next(inner.next().unwrap())?;
        left = TExpr::TBinOp(op_id, Box::new(left), Box::new(right));
    }

    Ok(left)
}

fn map_comp_op(op: &str) -> i64 {
    match op {
        "==" => 4,
        "!=" => 5,
        "<" => 6,
        ">" => 7,
        "<=" => 8,
        ">=" => 9,
        _ => 0, // Should not happen with valid grammar
    }
}

fn parse_unary(pair: Pair<Rule>) -> Result<TExpr, String> {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    if first.as_rule() == Rule::neg_op {
        let val = parse_unary(inner.next().unwrap())?;
        Ok(TExpr::TBinOp(1, Box::new(TExpr::TInt(0)), Box::new(val)))
    } else {
        parse_call(first)
    }
}

fn parse_call(pair: Pair<Rule>) -> Result<TExpr, String> {
    let mut inner = pair.into_inner();
    let atom_pair = inner.next().unwrap();
    let mut current = parse_atom(atom_pair)?;

    while let Some(args_pair) = inner.next() {
        let args = parse_arg_list(args_pair)?;
        
        // Builtin detection: if the callee is an identifier and it matches a builtin name
        if let TExpr::TVar(ref name) = current {
            if let Some(id) = map_builtin(name) {
                current = TExpr::TBuiltin(id, args);
                continue;
            }
        }
        
        current = TExpr::TApp(Box::new(current), args);
    }

    Ok(current)
}

fn parse_arg_list(pair: Pair<Rule>) -> Result<Vec<TExpr>, String> {
    pair.into_inner().map(parse_expr).collect()
}

fn parse_atom(pair: Pair<Rule>) -> Result<TExpr, String> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::int_lit => Ok(TExpr::TInt(inner.as_str().parse::<i64>().map_err(|e| e.to_string())?)),
        Rule::string_lit => {
            let s = inner.into_inner().next().map(|p| p.as_str()).unwrap_or("");
            Ok(TExpr::TStr(s.to_string()))
        }
        Rule::bool_lit => Ok(TExpr::TBool(inner.as_str() == "true")),
        Rule::ident => Ok(TExpr::TVar(inner.as_str().to_string())),
        Rule::list_lit => {
            let args = if let Some(arg_list) = inner.into_inner().next() {
                parse_arg_list(arg_list)?
            } else {
                vec![]
            };
            Ok(TExpr::TList(args))
        }
        Rule::expr => parse_expr(inner),
        _ => Err(format!("Unexpected atom rule: {:?}", inner.as_rule())),
    }
}

fn map_builtin(name: &str) -> Option<i64> {
    match name {
        "print" => Some(0),
        "fetch" => Some(1),
        "read_file" => Some(2),
        "write_file" => Some(3),
        "len" => Some(4),
        "str" => Some(5),
        "int" => Some(6),
        "concat" => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_int() {
        assert_eq!(parse("42").unwrap(), TExpr::TInt(42));
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(parse("\"hello\"").unwrap(), TExpr::TStr("hello".into()));
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse("true").unwrap(), TExpr::TBool(true));
        assert_eq!(parse("false").unwrap(), TExpr::TBool(false));
    }

    #[test]
    fn test_parse_var() {
        assert_eq!(parse("x").unwrap(), TExpr::TVar("x".into()));
    }

    #[test]
    fn test_parse_arithmetic() {
        // 2 + 3
        let expr = parse("2 + 3").unwrap();
        assert_eq!(expr, TExpr::TBinOp(0, Box::new(TExpr::TInt(2)), Box::new(TExpr::TInt(3))));
    }

    #[test]
    fn test_parse_precedence() {
        // 1 + 2 * 3 -> 1 + (2 * 3)
        let expr = parse("1 + 2 * 3").unwrap();
        assert_eq!(expr, TExpr::TBinOp(0,
            Box::new(TExpr::TInt(1)),
            Box::new(TExpr::TBinOp(2, Box::new(TExpr::TInt(2)), Box::new(TExpr::TInt(3))))
        ));
    }

    #[test]
    fn test_parse_let() {
        // let x = 5; x
        let expr = parse("let x = 5; x").unwrap();
        assert_eq!(expr, TExpr::TLet(
            "x".into(),
            Box::new(TExpr::TInt(5)),
            Box::new(TExpr::TVar("x".into()))
        ));
    }

    #[test]
    fn test_parse_if() {
        let expr = parse("if true then 1 else 2").unwrap();
        assert_eq!(expr, TExpr::TIf(
            Box::new(TExpr::TBool(true)),
            Box::new(TExpr::TInt(1)),
            Box::new(TExpr::TInt(2))
        ));
    }

    #[test]
    fn test_parse_lambda() {
        let expr = parse(r#"\x -> x"#).unwrap();
        assert_eq!(expr, TExpr::TLam(
            vec!["x".into()],
            Box::new(TExpr::TVar("x".into()))
        ));
    }

    #[test]
    fn test_parse_call() {
        let expr = parse("f(1, 2)").unwrap();
        assert_eq!(expr, TExpr::TApp(
            Box::new(TExpr::TVar("f".into())),
            vec![TExpr::TInt(1), TExpr::TInt(2)]
        ));
    }

    #[test]
    fn test_parse_builtin() {
        let expr = parse("print(42)").unwrap();
        assert_eq!(expr, TExpr::TBuiltin(0, vec![TExpr::TInt(42)]));
    }

    #[test]
    fn test_parse_list() {
        let expr = parse("[1, 2, 3]").unwrap();
        assert_eq!(expr, TExpr::TList(vec![TExpr::TInt(1), TExpr::TInt(2), TExpr::TInt(3)]));
    }

    #[test]
    fn test_parse_concat() {
        let expr = parse(r#""a" ++ "b""#).unwrap();
        assert_eq!(expr, TExpr::TBinOp(10,
            Box::new(TExpr::TStr("a".into())),
            Box::new(TExpr::TStr("b".into()))
        ));
    }

    #[test]
    fn test_parse_comparison() {
        let expr = parse("x == 0").unwrap();
        assert_eq!(expr, TExpr::TBinOp(4,
            Box::new(TExpr::TVar("x".into())),
            Box::new(TExpr::TInt(0))
        ));
    }

    #[test]
    fn test_parse_complex() {
        // let x = 2 + 3; x * 10
        let expr = parse("let x = 2 + 3; x * 10").unwrap();
        assert_eq!(expr, TExpr::TLet(
            "x".into(),
            Box::new(TExpr::TBinOp(0, Box::new(TExpr::TInt(2)), Box::new(TExpr::TInt(3)))),
            Box::new(TExpr::TBinOp(2, Box::new(TExpr::TVar("x".into())), Box::new(TExpr::TInt(10))))
        ));
    }
}