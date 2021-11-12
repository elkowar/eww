use eww_shared_util::Span;

use crate::SimplExpr;

use super::lexer::{LexicalError, Sp, StrLitSegment, Token};

pub fn b<T>(x: T) -> Box<T> {
    Box::new(x)
}

pub fn parse_stringlit(
    span: Span,
    mut segs: Vec<Sp<StrLitSegment>>,
) -> Result<SimplExpr, lalrpop_util::ParseError<usize, Token, LexicalError>> {
    let file_id = span.2;
    let parser = crate::simplexpr_parser::ExprParser::new();

    if segs.len() == 1 {
        let (lo, seg, hi) = segs.remove(0);
        let span = Span(lo, hi, file_id);
        match seg {
            StrLitSegment::Literal(lit) => Ok(SimplExpr::literal(span, lit)),
            StrLitSegment::Interp(toks) => {
                let token_stream = toks.into_iter().map(Ok);
                parser.parse(file_id, token_stream)
            }
        }
    } else {
        let elems = segs
            .into_iter()
            .filter_map(|(lo, segment, hi)| {
                let span = Span(lo, hi, file_id);
                match segment {
                    StrLitSegment::Literal(lit) if lit.is_empty() => None,
                    StrLitSegment::Literal(lit) => Some(Ok(SimplExpr::literal(span, lit))),
                    StrLitSegment::Interp(toks) => {
                        let token_stream = toks.into_iter().map(Ok);
                        Some(parser.parse(file_id, token_stream))
                    }
                }
            })
            .collect::<Result<Vec<SimplExpr>, _>>()?;
        Ok(SimplExpr::Concat(span, elems))
    }
}
