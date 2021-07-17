use crate::{
    ast::{Ast, AstIterator, AstType, Span},
    error::*,
    parser, spanned,
};
use itertools::Itertools;
use std::{
    collections::{HashMap, LinkedList},
    iter::FromIterator,
    str::FromStr,
};

type VarName = String;
type AttrValue = String;
type AttrName = String;

pub trait FromAst: Sized {
    fn from_ast(e: Ast) -> AstResult<Self>;
}

impl FromAst for Ast {
    fn from_ast(e: Ast) -> AstResult<Self> {
        Ok(e)
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
            let attrs = iter.expect_key_values()?;
            let children = iter.map(C::from_ast).collect::<AstResult<Vec<_>>>()?;
            Element { span, name, attrs, children }
        })
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::lexer;
    use insta;

    #[test]
    fn test() {
        let parser = parser::AstParser::new();
        insta::with_settings!({sort_maps => true}, {
            let lexer = lexer::Lexer::new(0, "(box :bar 12 :baz \"hi\" foo (bar))".to_string());
            insta::assert_debug_snapshot!(
                Element::<Ast, Ast>::from_ast(parser.parse(0, lexer).unwrap()).unwrap()
            );
        });
    }
}
