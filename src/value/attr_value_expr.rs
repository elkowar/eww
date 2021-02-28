use anyhow::*;
use std::collections::HashMap;

use super::*;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Operator {
    Equals,
    NotEquals,
    And,
    Or,
}

impl Operator {
    fn parse(s: &str) -> Result<Self> {
        use Operator::*;
        match s {
            "==" => Ok(Equals),
            "!=" => Ok(NotEquals),
            "&&" => Ok(And),
            "||" => Ok(Or),
            _ => bail!("{} is not a valid operator", s),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum AttrValueExpr {
    Literal(AttrValue),
    Ref(VarName),
    Op(Box<AttrValueExpr>, Operator, Box<AttrValueExpr>),
    Ternary(Box<AttrValueExpr>, Box<AttrValueExpr>, Box<AttrValueExpr>),
}

impl AttrValueExpr {
    pub fn map_terminals_into(self, f: impl Fn(Self) -> Self) -> Self {
        use AttrValueExpr::*;
        match self {
            Op(box a, op, box b) => Op(Box::new(f(a)), op, Box::new(f(b))),
            Ternary(box a, box b, box c) => Ternary(Box::new(f(a)), Box::new(f(b)), Box::new(f(c))),
            other => f(other),
        }
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    pub fn resolve_refs(self, variables: &HashMap<VarName, PrimitiveValue>) -> Result<Self> {
        use AttrValueExpr::*;
        match self {
            Literal(x) => Ok(AttrValueExpr::Literal(x)),
            Ref(ref name) => Ok(Literal(AttrValue::from_primitive(
                variables
                    .get(name)
                    .with_context(|| format!("Unknown variable {} referenced in {:?}", &name, &self))?
                    .clone(),
            ))),
            Op(box a, op, box b) => Ok(Op(
                Box::new(a.resolve_refs(variables)?),
                op,
                Box::new(b.resolve_refs(variables)?),
            )),
            Ternary(box a, box b, box c) => Ok(Ternary(
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
            Ref(name) => vec![name],
            Op(box a, _, box b) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs
            }
            Ternary(box a, box b, box c) => {
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
            AttrValueExpr::Ref(ref name) => Err(anyhow!(
                "Got unresolved variable {} while trying to evaluate expression {:?}",
                &name,
                &self
            )),
            AttrValueExpr::Op(a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                match op {
                    Operator::Equals => Ok(PrimitiveValue::from(a == b)),
                    Operator::NotEquals => Ok(PrimitiveValue::from(a != b)),
                    Operator::And => Ok(PrimitiveValue::from(a.as_bool()? && b.as_bool()?)),
                    Operator::Or => Ok(PrimitiveValue::from(a.as_bool()? || b.as_bool()?)),
                }
            }
            AttrValueExpr::Ternary(cond, yes, no) => {
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
    pub fn parse(s: &str) -> Result<Self> {
        use regex::Regex;
        use AttrValueExpr::*;
        let op_pattern: Regex = Regex::new(r#"\((.*) (.*) (.*)\)"#).unwrap();
        let ternary_pattern: Regex = Regex::new(r#"if (.+?) then (.+?) else (.+)"#).unwrap();
        let literal_pattern: Regex = Regex::new(r#"(".*"|\d+|true|false)"#).unwrap();
        dbg!("bruh????");

        if let Some(caps) = ternary_pattern.captures(s) {
            dbg!(&caps);
            Ok(Ternary(
                Box::new(AttrValueExpr::parse(&caps[1])?),
                Box::new(AttrValueExpr::parse(&caps[2])?),
                Box::new(AttrValueExpr::parse(&caps[3])?),
            ))
        } else if let Some(caps) = op_pattern.captures(s) {
            dbg!(&caps);
            Ok(Op(
                Box::new(AttrValueExpr::parse(&caps[1])?),
                Operator::parse(&caps[2])?,
                Box::new(AttrValueExpr::parse(&caps[3])?),
            ))
        } else if let Some(caps) = literal_pattern.captures(s) {
            dbg!("c");
            Ok(Literal(AttrValue::parse_string(&caps[1])))
        } else {
            bail!("Could not parse {} as valid expression", s);
        }
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
