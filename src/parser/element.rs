use super::ast::{Ast, AstIterator, AstType, Span};
use crate::{error::*, parser, spanned, value::AttrName};
use itertools::Itertools;
use simplexpr::ast::SimplExpr;
use std::{
    collections::{HashMap, LinkedList},
    iter::FromIterator,
    str::FromStr,
};

pub trait FromAst: Sized {
    fn from_ast(e: Ast) -> AstResult<Self>;
}

impl FromAst for Ast {
    fn from_ast(e: Ast) -> AstResult<Self> {
        Ok(e)
    }
}

impl FromAst for SimplExpr {
    fn from_ast(e: Ast) -> AstResult<Self> {
        match e {
            Ast::Symbol(span, x) => Ok(SimplExpr::VarRef(span.into(), x)),
            Ast::Value(span, x) => Ok(SimplExpr::Literal(span.into(), x)),
            Ast::SimplExpr(span, x) => Ok(x),
            _ => Err(AstError::NotAValue(Some(e.span()), e.expr_type())),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Element<C, A> {
    name: String,
    attrs: HashMap<AttrName, A>,
    children: Vec<C>,
    span: Span,
}

impl<C: FromAst, A: FromAst> FromAst for Element<C, A> {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = AstIterator::new(list.into_iter());
            let (_, name) = iter.expect_symbol()?;
            let attrs = iter.expect_key_values()?.into_iter().map(|(k, v)| (AttrName(k), v)).collect();
            let children = iter.map(C::from_ast).collect::<AstResult<Vec<_>>>()?;
            Element { span, name, attrs, children }
        })
    }
}

#[cfg(test)]
mod test {

    use super::super::{
        ast::Ast,
        element::{Element, FromAst},
        lexer,
    };

    use insta;

    #[test]
    fn test() {
        let parser = super::parser::parser::AstParser::new();
        insta::with_settings!({sort_maps => true}, {
            let lexer = lexer::Lexer::new(0, "(box :bar 12 :baz \"hi\" foo (bar))".to_string());
            insta::assert_debug_snapshot!(
                Element::<Ast, Ast>::from_ast(parser.parse(0, lexer).unwrap()).unwrap()
            );
        });
    }
}
