use codespan_reporting::{diagnostic, files};
use simplexpr::dynval;

use diagnostic::*;

use crate::error::AstError;

use super::parser::parse_error;
use eww_shared_util::{AttrName, Span, VarName};

fn span_to_secondary_label(span: Span) -> Label<usize> {
    Label::secondary(span.2, span.0..span.1)
}

macro_rules! gen_diagnostic {
    (
        $(msg = $msg:expr)?
        $(, label = $span:expr $(=> $label:expr)?)?
        $(, note = $note:expr)? $(,)?
    ) => {
        Diagnostic::error()
            $(.with_message($msg.to_string()))?
            $(.with_labels(vec![
                Label::primary($span.2, $span.0..$span.1)
                    $(.with_message($label))?
            ]))?
            $(.with_notes(vec![$note]))?
    };
    ($msg:expr $(, $span:expr $(,)?)?) => {{
        Diagnostic::error()
            .with_message($msg.to_string())
            $(.with_labels(vec![Label::primary($span.2, $span.0..$span.1)]))?
    }};
}

pub trait DiagnosticExt: Sized {
    fn with_opt_label(self, label: Option<Label<usize>>) -> Self;
}

impl DiagnosticExt for Diagnostic<usize> {
    fn with_opt_label(self, label: Option<Label<usize>>) -> Self {
        if let Some(label) = label {
            self.with_labels(vec![label])
        } else {
            self
        }
    }
}

pub trait ToDiagnostic {
    fn to_diagnostic(&self) -> Diagnostic<usize>;
}

impl ToDiagnostic for AstError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        if let AstError::ValidationError(error) = self {
            match error {
                crate::config::validate::ValidationError::UnknownWidget(span, name) => gen_diagnostic! {
                    msg = format!("No widget named {} exists", name),
                    label = span => "Used here",
                },
                crate::config::validate::ValidationError::MissingAttr { widget_name, arg_name, arg_list_span, use_span } => {
                    let diag = gen_diagnostic! {
                        msg = format!("{}", error),
                    };
                    diag.with_opt_label(Some(span_to_secondary_label(*use_span).with_message("Argument missing here")))
                        .with_opt_label(arg_list_span.map(|s| span_to_secondary_label(s).with_message("but is required here")))
                }
            }
        } else if let Some(span) = self.get_span() {
            match self {
                AstError::UnknownToplevel(_, name) => gen_diagnostic!(format!("{}", self), span),
                AstError::MissingNode(_) => gen_diagnostic! {
                    msg = "Expected another element",
                    label = span => "Expected another element here",
                },

                AstError::WrongExprType(_, expected, actual) => gen_diagnostic! {
                    msg = "Wrong type of expression",
                    label = span => format!("Expected a `{}` here", expected),
                    note = format!("Expected: {}\nGot: {}", expected, actual),
                },
                AstError::NotAValue(_, actual) => gen_diagnostic! {
                    msg = format!("Expected value, but got `{}`", actual),
                    label = span => "Expected some value here",
                    note = format!("Got: {}", actual),
                },

                AstError::ParseError { file_id, source } => lalrpop_error_to_diagnostic(source, span, |error| match error {
                    parse_error::ParseError::SimplExpr(_, error) => simplexpr_error_to_diagnostic(error, span),
                    parse_error::ParseError::LexicalError(span) => lexical_error_to_diagnostic(*span),
                }),
                AstError::MismatchedElementName(_, expected, got) => gen_diagnostic! {
                    msg = format!("Expected element `{}`, but found `{}`", expected, got),
                    label = span => format!("Expected `{}` here", expected),
                    note = format!("Expected: {}\nGot: {}", expected, got),
                },
                AstError::ConversionError(err) => conversion_error_to_diagnostic(err, span),
                AstError::Other(_, source) => gen_diagnostic!(source, span),
                AstError::AttrError(source) => gen_diagnostic!(source, span),
                AstError::IncludedFileNotFound(include) => gen_diagnostic!(
                    msg = format!("Included file `{}` not found", include.path),
                    label = include.path_span => "Included here",
                ),

                AstError::ValidationError(_) => todo!(),
            }
        } else {
            Diagnostic::error().with_message(format!("{}", self))
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

// TODO this needs a lot of improvement
pub fn simplexpr_error_to_diagnostic(error: &simplexpr::error::Error, span: Span) -> Diagnostic<usize> {
    use simplexpr::error::Error::*;
    match error {
        ParseError { source, .. } => lalrpop_error_to_diagnostic(source, span, move |error| lexical_error_to_diagnostic(span)),
        ConversionError(error) => conversion_error_to_diagnostic(error, span),
        Eval(error) => eval_error_to_diagnostic(error, span),
        Other(error) => gen_diagnostic!(error, span),
        Spanned(span, error) => gen_diagnostic!(error, span),
    }
}

// TODO this needs a lot of improvement
pub fn eval_error_to_diagnostic(error: &simplexpr::eval::EvalError, span: Span) -> Diagnostic<usize> {
    gen_diagnostic!(error, error.span().unwrap_or(span))
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
