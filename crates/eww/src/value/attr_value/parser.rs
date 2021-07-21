use super::*;

use nom::{
    branch::*,
    bytes::complete::{tag, take_while},
    character::complete::{multispace0 as multispace, *},
    combinator::{map, map_res, *},
    error::{context, ParseError, VerboseError},
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, *},
    IResult, Parser,
};

use super::super::*;

fn ws<'a, P, O, E: ParseError<&'a str>>(p: P) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    P: Parser<&'a str, O, E>,
{
    delimited(multispace, p, multispace)
}

fn parse_num(i: &str) -> IResult<&str, f64, VerboseError<&str>> {
    let (i, neg) = opt(tag("-"))(i)?;
    let (i, num): (_, f64) = map_res(take_while(|c: char| c.is_numeric() || c == '.'), |n: &str| n.parse::<f64>())(i)?;
    Ok((i, if neg.is_some() { -num } else { num }))
}

fn parse_bool(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
    alt((tag("true"), tag("false")))(i)
}

fn parse_literal(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
    alt((parse_bool, parse_stringlit, recognize(parse_num)))(i)
}

fn parse_stringlit(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
    alt((delimited(tag("'"), take_while(|c| c != '\''), tag("'")), delimited(tag("\""), take_while(|c| c != '"'), tag("\""))))(i)
}

fn parse_identifier(i: &str) -> IResult<&str, &str, VerboseError<&str>> {
    verify(recognize(pair(alt((alpha1, tag("_"), tag("-"))), many0(alt((alphanumeric1, tag("_"), tag("-")))))), |x| {
        !["if", "then", "else"].contains(x)
    })(i)
}

fn parse_unary_op(i: &str) -> IResult<&str, UnaryOp, VerboseError<&str>> {
    value(UnaryOp::Not, tag("!"))(i)
}

fn parse_function_call(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, name) = take_while(|c: char| c.is_ascii_alphanumeric() || c == '_')(i)?;
    let (i, args) = delimited(tag("("), separated_list0(tag(","), ws(parse_expr)), tag(")"))(i)?;
    Ok((i, AttrValExpr::FunctionCall(name.to_string(), args)))
}

/////////////////
// actual tree //
/////////////////

fn parse_factor(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, unary_op) = opt(parse_unary_op)(i)?;
    let (i, factor) = alt((
        context("expression", ws(delimited(tag("("), parse_expr, tag(")")))),
        context("if-expression", ws(parse_ifelse)),
        context("function-call", ws(parse_function_call)),
        context("literal", map(ws(parse_literal), |x| AttrValExpr::Literal(AttrVal::parse_string(x)))),
        context("identifier", map(ws(parse_identifier), |x| AttrValExpr::VarRef(VarName(x.to_string())))),
    ))(i)?;
    Ok((
        i,
        match unary_op {
            Some(op) => AttrValExpr::UnaryOp(op, box factor),
            None => factor,
        },
    ))
}

fn parse_object_index(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, initial) = parse_factor(i)?;
    let (i, remainder) = many0(alt((
        delimited(tag("["), ws(parse_expr), tag("]")),
        map(preceded(tag("."), parse_identifier), |x| AttrValExpr::Literal(AttrVal::from_primitive(x))),
    )))(i)?;
    let indexes = remainder.into_iter().fold(initial, |acc, index| AttrValExpr::JsonAccess(box acc, box index));
    Ok((i, indexes))
}

fn parse_term3(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, initial) = parse_object_index(i)?;
    let (i, remainder) = many0(alt((
        map(preceded(tag("*"), parse_object_index), |x| (BinOp::Times, x)),
        map(preceded(tag("/"), parse_object_index), |x| (BinOp::Div, x)),
        map(preceded(tag("%"), parse_object_index), |x| (BinOp::Mod, x)),
    )))(i)?;

    let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| AttrValExpr::BinOp(box acc, op, box expr));

    Ok((i, exprs))
}
fn parse_term2(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, initial) = parse_term3(i)?;
    let (i, remainder) = many0(alt((
        map(preceded(tag("+"), parse_term3), |x| (BinOp::Plus, x)),
        map(preceded(tag("-"), parse_term3), |x| (BinOp::Minus, x)),
    )))(i)?;

    let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| AttrValExpr::BinOp(box acc, op, box expr));

    Ok((i, exprs))
}

fn parse_term1(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, initial) = parse_term2(i)?;
    let (i, remainder) = many0(alt((
        map(preceded(tag("=="), parse_term2), |x| (BinOp::Equals, x)),
        map(preceded(tag("!="), parse_term2), |x| (BinOp::NotEquals, x)),
        map(preceded(tag(">"), parse_term2), |x| (BinOp::GT, x)),
        map(preceded(tag("<"), parse_term2), |x| (BinOp::LT, x)),
        map(preceded(tag("=~"), parse_term2), |x| (BinOp::RegexMatch, x)),
    )))(i)?;

    let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| AttrValExpr::BinOp(box acc, op, box expr));

    Ok((i, exprs))
}
pub fn parse_expr(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, initial) = parse_term1(i)?;
    let (i, remainder) = many0(alt((
        map(preceded(tag("&&"), parse_term1), |x| (BinOp::And, x)),
        map(preceded(tag("||"), parse_term1), |x| (BinOp::Or, x)),
        map(preceded(tag("?:"), parse_term1), |x| (BinOp::Elvis, x)),
    )))(i)?;

    let exprs = remainder.into_iter().fold(initial, |acc, (op, expr)| AttrValExpr::BinOp(box acc, op, box expr));

    Ok((i, exprs))
}

fn parse_ifelse(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    let (i, _) = tag("if")(i)?;
    let (i, a) = context("condition", ws(parse_expr))(i)?;
    let (i, _) = tag("then")(i)?;
    let (i, b) = context("true-case", ws(parse_expr))(i)?;
    let (i, _) = tag("else")(i)?;
    let (i, c) = context("false-case", ws(parse_expr))(i)?;
    Ok((i, AttrValExpr::IfElse(box a, box b, box c)))
}

pub fn parse(i: &str) -> IResult<&str, AttrValExpr, VerboseError<&str>> {
    complete(parse_expr)(i)
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn test_parser() {
        use self::{BinOp::*, UnaryOp::*};
        use AttrValExpr::*;

        assert_eq!(("", 12.22f64), parse_num("12.22").unwrap());
        assert_eq!(Literal(AttrVal::from_primitive("12")), AttrValExpr::parse("12").unwrap());
        assert_eq!(UnaryOp(Not, box Literal(AttrVal::from_primitive("false"))), AttrValExpr::parse("!false").unwrap());
        assert_eq!(
            BinOp(box Literal(AttrVal::from_primitive("12")), Plus, box Literal(AttrVal::from_primitive("2"))),
            AttrValExpr::parse("12 + 2").unwrap()
        );
        assert_eq!(
            BinOp(
                box FunctionCall(
                    "test".to_string(),
                    vec![
                        JsonAccess(box VarRef(VarName("foo".to_string())), box Literal(AttrVal::from_primitive("hi"))),
                        Literal(AttrVal::from_primitive("ho")),
                    ]
                ),
                Times,
                box Literal(AttrVal::from_primitive(2))
            ),
            AttrValExpr::parse(r#"(test(foo["hi"], ("ho")) * 2)"#).unwrap()
        );
        assert_eq!(
            UnaryOp(Not, box BinOp(box Literal(AttrVal::from_primitive("1")), Equals, box Literal(AttrVal::from_primitive("2")))),
            AttrValExpr::parse("!(1 == 2)").unwrap()
        );
        assert_eq!(
            IfElse(
                box VarRef(VarName("a".to_string())),
                box VarRef(VarName("b".to_string())),
                box VarRef(VarName("c".to_string())),
            ),
            AttrValExpr::parse("if a then b else c").unwrap()
        );
        assert_eq!(
            JsonAccess(
                box VarRef(VarName("array".to_string())),
                box BinOp(box Literal(AttrVal::from_primitive("1")), Plus, box Literal(AttrVal::from_primitive("2")))
            ),
            AttrValExpr::parse(r#"(array)[1+2]"#).unwrap()
        );
        assert_eq!(
            JsonAccess(
                box JsonAccess(
                    box VarRef(VarName("object".to_string())),
                    box Literal(AttrVal::from_primitive("field".to_string())),
                ),
                box Literal(AttrVal::from_primitive("field2".to_string())),
            ),
            AttrValExpr::parse(r#"object.field.field2"#).unwrap()
        );
    }
    #[test]
    fn test_complex() {
        let parsed =
            AttrValExpr::parse(r#"if hi > 12 + 2 * 2 && 12 == 15 then "foo" else if !true then 'hi' else "{{bruh}}""#).unwrap();

        assert_eq!(
            r#"(if ((hi > ("12" + ("2" * "2"))) && ("12" == "15")) then "foo" else (if !"true" then "hi" else "{{bruh}}"))"#,
            format!("{}", parsed),
        )
    }
}
