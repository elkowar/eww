use codespan_reporting::{diagnostic, files};
use simplexpr::dynval;

use crate::{ast::Span, error::AstError, parse_error};
use diagnostic::*;

fn span_to_label(span: Span) -> Label<usize> {
    Label::primary(span.2, span.0..span.1)
}

pub trait ToDiagnostic {
    fn to_diagnostic(&self, files: &files::SimpleFiles<&str, &str>) -> Diagnostic<usize>;
}

impl ToDiagnostic for AstError {
    fn to_diagnostic(&self, files: &files::SimpleFiles<&str, &str>) -> Diagnostic<usize> {
        let diag = Diagnostic::error();
        if let Some(span) = self.get_span() {
            use lalrpop_util::ParseError::*;
            match self {
                AstError::InvalidDefinition(_) => todo!(),

                AstError::MissingNode(_, expected) => diag
                    .with_message(format!("Missing {}", expected))
                    .with_labels(vec![span_to_label(span).with_message(format!("Expected `{}` here", expected))]),

                AstError::WrongExprType(_, expected, actual) => diag
                    .with_message("Wrong type of expression")
                    .with_notes(vec![format!("Expected: {}\nGot: {}", expected, actual)])
                    .with_labels(vec![span_to_label(span).with_message(format!("Expected a `{}` here", expected))]),

                AstError::ParseError { file_id, source } => {
                    lalrpop_error_to_diagnostic(source, diag, span, move |diag, error| match error {
                        parse_error::ParseError::SimplExpr(_, error) => simplexpr_error_to_diagnostic(error, diag, span),
                        parse_error::ParseError::LexicalError(_) => diag
                            .with_message("Invalid token")
                            .with_labels(vec![span_to_label(span).with_message("Invalid token")]),
                    })
                }
            }
        } else {
            diag.with_message(format!("{}", self))
        }
    }
}

fn lalrpop_error_to_diagnostic<T: std::fmt::Display, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    diag: Diagnostic<usize>,
    span: Span,
    handle_user_error: impl FnOnce(Diagnostic<usize>, &E) -> Diagnostic<usize>,
) -> Diagnostic<usize> {
    use lalrpop_util::ParseError::*;
    match error {
        InvalidToken { location } => diag.with_message("Invalid token").with_labels(vec![span_to_label(span)]),
        UnrecognizedEOF { location, expected } => diag
            .with_message("Input ended unexpectedly. Check if you have any unclosed delimiters")
            .with_labels(vec![span_to_label(span)]),
        UnrecognizedToken { token, expected } => diag
            .with_message(format!("Unexpected token `{}` encoutered", token.1))
            .with_labels(vec![span_to_label(span).with_message("Token unexpected")]),

        ExtraToken { token } => diag.with_message(format!("Extra token encountered: `{}`", token.1)),
        User { error } => handle_user_error(diag, error),
    }
}

fn simplexpr_error_to_diagnostic(error: &simplexpr::error::Error, diag: Diagnostic<usize>, span: Span) -> Diagnostic<usize> {
    match error {
        simplexpr::error::Error::ParseError { source } => lalrpop_error_to_diagnostic(source, diag, span, move |diag, error| {
            diag.with_message("Invalid token").with_labels(vec![span_to_label(span).with_message("Invalid token")])
        }),
        simplexpr::error::Error::ConversionError(dynval::ConversionError { value, target_type, source }) => diag
            .with_message(format!("{}", error))
            .with_labels(vec![span_to_label(span).with_message(format!("{} is not of type `{}`", value, target_type))])
            .with_notes(source.as_ref().map(|x| vec![format!("{}", x)]).unwrap_or_default()),
        simplexpr::error::Error::Spanned(..) => todo!(),
        simplexpr::error::Error::Eval(error) => diag.with_message(format!("{}", error)).with_labels(vec![span_to_label(span)]),
        simplexpr::error::Error::Other(error) => diag.with_message(format!("{}", error)).with_labels(vec![span_to_label(span)]),
    }
}
