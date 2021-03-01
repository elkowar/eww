use anyhow::*;
use std::collections::HashMap;

use super::*;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Op {
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    Equals,
    NotEquals,
    And,
    Or,
    GT,
    LT,
}
impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::Plus => write!(f, "+"),
            Op::Minus => write!(f, "-"),
            Op::Times => write!(f, "*"),
            Op::Div => write!(f, "/"),
            Op::Mod => write!(f, "%"),
            Op::Equals => write!(f, "=="),
            Op::NotEquals => write!(f, "!="),
            Op::And => write!(f, "&&"),
            Op::Or => write!(f, "||"),
            Op::GT => write!(f, ">"),
            Op::LT => write!(f, "<"),
        }
    }
}

impl Op {
    fn parse(s: &str) -> Result<Self> {
        use Op::*;
        match s {
            "==" => Ok(Equals),
            "!=" => Ok(NotEquals),
            "&&" => Ok(And),
            "||" => Ok(Or),
            _ => bail!("{} is not a valid operator", s),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrValueExpr {
    Literal(AttrValue),
    VarRef(VarName),
    BinOp(Box<AttrValueExpr>, Op, Box<AttrValueExpr>),
    IfElse(Box<AttrValueExpr>, Box<AttrValueExpr>, Box<AttrValueExpr>),
}

impl std::fmt::Display for AttrValueExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttrValueExpr::VarRef(x) => write!(f, "{}", x),
            AttrValueExpr::Literal(x) => write!(f, "\"{:?}\"", x),
            AttrValueExpr::BinOp(l, op, r) => write!(f, "({} {} {})", l, op, r),
            AttrValueExpr::IfElse(a, b, c) => write!(f, "[if {} then {} else {}]", a, b, c),
        }
    }
}

impl std::fmt::Debug for AttrValueExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl AttrValueExpr {
    pub fn map_terminals_into(self, f: impl Fn(Self) -> Self) -> Self {
        use AttrValueExpr::*;
        match self {
            BinOp(box a, op, box b) => BinOp(Box::new(f(a)), op, Box::new(f(b))),
            IfElse(box a, box b, box c) => IfElse(Box::new(f(a)), Box::new(f(b)), Box::new(f(c))),
            other => f(other),
        }
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    pub fn resolve_refs(self, variables: &HashMap<VarName, PrimitiveValue>) -> Result<Self> {
        use AttrValueExpr::*;
        match self {
            Literal(x) => Ok(AttrValueExpr::Literal(x)),
            VarRef(ref name) => Ok(Literal(AttrValue::from_primitive(
                variables
                    .get(name)
                    .with_context(|| format!("Unknown variable {} referenced in {:?}", &name, &self))?
                    .clone(),
            ))),
            BinOp(box a, op, box b) => Ok(BinOp(
                Box::new(a.resolve_refs(variables)?),
                op,
                Box::new(b.resolve_refs(variables)?),
            )),
            IfElse(box a, box b, box c) => Ok(IfElse(
                Box::new(a.resolve_refs(variables)?),
                Box::new(b.resolve_refs(variables)?),
                Box::new(c.resolve_refs(variables)?),
            )),
        }
    }

    pub fn var_refs(&self) -> Vec<&VarName> {
        use AttrValueExpr::*;
        match self {
            Literal(_) => vec![],
            VarRef(name) => vec![name],
            BinOp(box a, _, box b) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs
            }
            IfElse(box a, box b, box c) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs.append(&mut c.var_refs());
                refs
            }
        }
    }

    pub fn eval(self, values: &HashMap<VarName, PrimitiveValue>) -> Result<PrimitiveValue> {
        match self {
            AttrValueExpr::Literal(x) => x.resolve_fully(&values),
            AttrValueExpr::VarRef(ref name) => values.get(name).cloned().context(format!(
                "Got unresolved variable {} while trying to evaluate expression {:?}",
                &name, &self
            )),
            AttrValueExpr::BinOp(a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                Ok(match op {
                    Op::Equals => PrimitiveValue::from(a == b),
                    Op::NotEquals => PrimitiveValue::from(a != b),
                    Op::And => PrimitiveValue::from(a.as_bool()? && b.as_bool()?),
                    Op::Or => PrimitiveValue::from(a.as_bool()? || b.as_bool()?),

                    Op::Plus => PrimitiveValue::from(a.as_f64()? + b.as_f64()?),
                    Op::Minus => PrimitiveValue::from(a.as_f64()? - b.as_f64()?),
                    Op::Times => PrimitiveValue::from(a.as_f64()? * b.as_f64()?),
                    Op::Div => PrimitiveValue::from(a.as_f64()? / b.as_f64()?),
                    Op::Mod => PrimitiveValue::from(a.as_f64()? % b.as_f64()?),
                    Op::GT => PrimitiveValue::from(a.as_f64()? > b.as_f64()?),
                    Op::LT => PrimitiveValue::from(a.as_f64()? < b.as_f64()?),
                })
            }
            AttrValueExpr::IfElse(cond, yes, no) => {
                if cond.eval(values)?.as_bool()? {
                    yes.eval(values)
                } else {
                    no.eval(values)
                }
            }
        }
    }

    // TODO this is way too fucked right now
    // i.e. variable names are just not supported, pretty much. that needs to change.
    // REEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
    // pub fn parse(s: &str) -> Result<Self> {
    // use regex::Regex;
    // use AttrValueExpr::*;
    // let op_pattern: Regex = Regex::new(r#"\((.*) (.*) (.*)\)"#).unwrap();
    // let ternary_pattern: Regex = Regex::new(r#"if (.+?) then (.+?) else (.+)"#).unwrap();
    // let literal_pattern: Regex = Regex::new(r#"(".*"|\d+|true|false)"#).unwrap();
    // dbg!("bruh????");

    // if let Some(caps) = ternary_pattern.captures(s) {
    // dbg!(&caps);
    // Ok(IfElse(
    // Box::new(AttrValueExpr::parse(&caps[1])?),
    // Box::new(AttrValueExpr::parse(&caps[2])?),
    // Box::new(AttrValueExpr::parse(&caps[3])?),
    //))
    //} else if let Some(caps) = op_pattern.captures(s) {
    // dbg!(&caps);
    // Ok(BinOp(
    // Box::new(AttrValueExpr::parse(&caps[1])?),
    // Op::parse(&caps[2])?,
    // Box::new(AttrValueExpr::parse(&caps[3])?),
    //))
    //} else if let Some(caps) = literal_pattern.captures(s) {
    // dbg!("c");
    // Ok(Literal(AttrValue::parse_string(&caps[1])))
    //} else {
    // bail!("Could not parse {} as valid expression", s);
    //}

    pub fn parse<'a>(s: &'a str) -> Result<Self> {
        let parsed = match parser::parse(s) {
            Ok((_, x)) => Ok(x),
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => Err(anyhow!(nom::error::convert_error(s, e))),
            Err(nom::Err::Incomplete(_)) => Err(anyhow!("Parsing incomplete")),
        };
        parsed.context("Failed to parse expression")
    }
}

mod parser {
    use super::*;

    use nom::{
        branch::*,
        bytes::complete::{tag, take_while},
        character::complete::{multispace0 as multispace, *},
        combinator::{map, map_res, *},
        error::{context, ParseError, VerboseError},
        multi::many0,
        sequence::{delimited, preceded, *},
        IResult, Parser,
    };

    fn ws<'a, P, O, E: ParseError<&'a str>>(p: P) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
    where
        P: Parser<&'a str, O, E>,
    {
        delimited(multispace, p, multispace)
    }

    fn parse_num(i: &str) -> IResult<&str, i32, VerboseError<&str>> {
        alt((
            map_res(digit1, |s: &str| s.parse::<i32>()),
            map_res(preceded(tag("-"), digit1), |s: &str| s.parse::<i32>().map(|x| x * -1)),
        ))(i)
    }

    fn parse_stringlit(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
        delimited(tag("\""), take_while(|c| c != '"'), tag("\""))(i)
    }

    fn parse_bool(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
        alt((tag("true"), tag("false")))(i)
    }

    fn parse_literal(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
        alt((parse_bool, parse_stringlit, recognize(parse_num)))(i)
    }

    pub fn parse_identifier(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
        verify(
            recognize(pair(
                alt((alpha1, tag("_"), tag("-"))),
                many0(alt((alphanumeric1, tag("_"), tag("-")))),
            )),
            |x| !["if", "then", "else"].contains(x),
        )(input)
    }

    /////////////////
    // actual tree //
    /////////////////

    fn parse_factor(i: &str) -> IResult<&str, AttrValueExpr, VerboseError<&str>> {
        alt((
            context("expression", ws(delimited(tag("("), parse_expr, tag(")")))),
            context("if-expression", ws(parse_ifelse)),
            context(
                "literal",
                map(ws(parse_literal), |x| AttrValueExpr::Literal(AttrValue::parse_string(x))),
            ),
            context(
                "identifier",
                map(ws(parse_identifier), |x| AttrValueExpr::VarRef(VarName(x.to_string()))),
            ),
        ))(i)
    }
    fn parse_term3(i: &str) -> IResult<&str, AttrValueExpr, VerboseError<&str>> {
        let (i, initial) = parse_factor(i)?;
        let (i, remainder) = many0(alt((
            map(preceded(tag("*"), parse_factor), |x| (Op::Times, x)),
            map(preceded(tag("/"), parse_factor), |x| (Op::Div, x)),
            map(preceded(tag("%"), parse_factor), |x| (Op::Mod, x)),
        )))(i)?;

        let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| {
            AttrValueExpr::BinOp(Box::new(acc), op, Box::new(expr))
        });

        Ok((i, exprs))
    }
    fn parse_term2(i: &str) -> IResult<&str, AttrValueExpr, VerboseError<&str>> {
        let (i, initial) = parse_term3(i)?;
        let (i, remainder) = many0(alt((
            map(preceded(tag("+"), parse_term3), |x| (Op::Plus, x)),
            map(preceded(tag("-"), parse_term3), |x| (Op::Minus, x)),
        )))(i)?;

        let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| {
            AttrValueExpr::BinOp(Box::new(acc), op, Box::new(expr))
        });

        Ok((i, exprs))
    }

    fn parse_term1(i: &str) -> IResult<&str, AttrValueExpr, VerboseError<&str>> {
        let (i, initial) = parse_term2(i)?;
        let (i, remainder) = many0(alt((
            map(preceded(tag("=="), parse_term2), |x| (Op::Equals, x)),
            map(preceded(tag("!="), parse_term2), |x| (Op::NotEquals, x)),
            map(preceded(tag(">"), parse_term2), |x| (Op::GT, x)),
            map(preceded(tag("<"), parse_term2), |x| (Op::LT, x)),
        )))(i)?;

        let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| {
            AttrValueExpr::BinOp(Box::new(acc), op, Box::new(expr))
        });

        Ok((i, exprs))
    }
    pub fn parse_expr(i: &str) -> IResult<&str, AttrValueExpr, VerboseError<&str>> {
        let (i, initial) = parse_term1(i)?;
        let (i, remainder) = many0(alt((
            map(preceded(tag("&&"), parse_term1), |x| (Op::And, x)),
            map(preceded(tag("||"), parse_term1), |x| (Op::Or, x)),
        )))(i)?;

        let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| {
            AttrValueExpr::BinOp(Box::new(acc), op, Box::new(expr))
        });

        Ok((i, exprs))
    }

    fn parse_ifelse(i: &str) -> IResult<&str, AttrValueExpr, VerboseError<&str>> {
        let (i, _) = tag("if")(i)?;
        let (i, a) = context("condition", ws(parse_expr))(i)?;
        let (i, _) = tag("then")(i)?;
        let (i, b) = context("true-case", ws(parse_expr))(i)?;
        let (i, _) = tag("else")(i)?;
        let (i, c) = context("false-case", ws(parse_expr))(i)?;
        Ok((i, AttrValueExpr::IfElse(Box::new(a), Box::new(b), Box::new(c))))
    }

    pub fn parse<'a>(i: &'a str) -> IResult<&'a str, AttrValueExpr, VerboseError<&'a str>> {
        complete(parse_expr)(i)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test() {
        let x = AttrValueExpr::parse(r#"if (12 == 15) then "foo" else true"#).unwrap();
        // let x = AttrValueExpr::parse(r#"(12 == 15)"#).unwrap();
        // let x = AttrValueExpr::parse(r#"if true then 12 else 14"#).unwrap();
        dbg!(&x);

        panic!("fuck :<");
    }
}
