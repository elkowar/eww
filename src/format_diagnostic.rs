use codespan_reporting::{diagnostic, files};
use simplexpr::dynval;

use diagnostic::*;

use crate::error::AstError;

use super::parser::{ast::Span, parse_error};

macro_rules! gen_diagnostic {
    (
        $(msg = $msg:expr)?
        $(, label = $span:expr $(=> $label:expr)?)?
        $(, note = $note:expr)? $(,)?
    ) => {
        Diagnostic::error()
            $(.with_message($msg))?
            $(.with_labels(vec![
                    Label::primary($span.2, $span.0..$span.1)
                        $(.with_message($label))?
            ]))?
            $(.with_notes(vec![$note]))?
    };
    ($msg:expr $(, $span:expr $(,)?)?) => {{
        Diagnostic::error()
            .with_message($msg)
            $(.with_labels(vec![Label::primary($span.2, $span.0..$span.1)]))?
    }};
}

pub trait ToDiagnostic {
    fn to_diagnostic(&self) -> Diagnostic<usize>;
}

impl ToDiagnostic for AstError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        let diag = Diagnostic::error();
        if let Some(span) = self.get_span() {
            use lalrpop_util::ParseError::*;
            match self {
                AstError::InvalidDefinition(_) => todo!(),

                AstError::MissingNode(_, expected) => gen_diagnostic! {
                    msg = format!("Missing {}", expected),
                    label = span => format!("Expected `{}` here", expected),
                },

                AstError::WrongExprType(_, expected, actual) => gen_diagnostic! {
                    msg = "Wrong type of expression",
                    label = span => format!("Expected a `{}` here", expected),
                    note = format!("Expected: {}\nGot: {}", expected, actual),
                },

                AstError::ParseError { file_id, source } => lalrpop_error_to_diagnostic(source, span, |error| match error {
                    parse_error::ParseError::SimplExpr(_, error) => simplexpr_error_to_diagnostic(error, span),
                    parse_error::ParseError::LexicalError(_) => lexical_error_to_diagnostic(span),
                }),
            }
        } else {
            diag.with_message(format!("{}", self))
        }
    }
}

fn lalrpop_error_to_diagnostic<T: std::fmt::Display, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    span: Span,
    handle_user_error: impl FnOnce(&E) -> Diagnostic<usize>,
) -> Diagnostic<usize> {
    use lalrpop_util::ParseError::*;
    match error {
        InvalidToken { location } => gen_diagnostic! { msg = "Invalid token", label = span },
        UnrecognizedEOF { location, expected } => gen_diagnostic! {
            msg = "Input ended unexpectedly. Check if you have any unclosed delimiters",
            label = span
        },
        UnrecognizedToken { token, expected } => gen_diagnostic! {
            msg = format!("Unexpected token `{}` encountered", token.1),
            label = span => "Token unexpected",
        },
        ExtraToken { token } => gen_diagnostic!(format!("Extra token encountered: `{}`", token.1)),
        User { error } => handle_user_error(error),
    }
}

fn simplexpr_error_to_diagnostic(error: &simplexpr::error::Error, span: Span) -> Diagnostic<usize> {
    use simplexpr::error::Error::*;
    match error {
        ParseError { source } => lalrpop_error_to_diagnostic(source, span, move |error| lexical_error_to_diagnostic(span)),
        ConversionError(error) => conversion_error_to_diagnostic(error, span),
        Eval(error) => gen_diagnostic!(format!("{}", error), span),
        Other(error) => gen_diagnostic!(format!("{}", error), span),
        Spanned(_, error) => gen_diagnostic!(format!("{}", error), span),
    }
}

fn conversion_error_to_diagnostic(error: &dynval::ConversionError, span: Span) -> Diagnostic<usize> {
    let diag = gen_diagnostic! {
        msg = format!("{}", error),
        label = span => format!("{} is not of type `{}`", error.value, error.target_type),
    };
    diag.with_notes(error.source.as_ref().map(|x| vec![format!("{}", x)]).unwrap_or_default())
}

fn lexical_error_to_diagnostic(span: Span) -> Diagnostic<usize> {
    gen_diagnostic! {
        msg = "Invalid token",
        label = span => "Invalid token"
    }
}
