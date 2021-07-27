use codespan_reporting::{diagnostic, files};
use simplexpr::dynval;

use diagnostic::*;

use crate::{
    config::{attributes::AttrError, validate::ValidationError},
    error::{get_parse_error_span, AstError},
};

use super::parser::parse_error;
use eww_shared_util::{AttrName, Span, Spanned, VarName};

fn span_to_primary_label(span: Span) -> Label<usize> {
    Label::primary(span.2, span.0..span.1)
}
fn span_to_secondary_label(span: Span) -> Label<usize> {
    Label::secondary(span.2, span.0..span.1)
}

#[macro_export]
macro_rules! gen_diagnostic {
    (
        $(msg = $msg:expr)?
        $(, label = $span:expr $(=> $label:expr)?)?
        $(, note = $note:expr)? $(,)?
    ) => {
        ::codespan_reporting::diagnostic::Diagnostic::error()
            $(.with_message($msg.to_string()))?
            $(.with_labels(vec![
                ::codespan_reporting::diagnostic::Label::primary($span.2, $span.0..$span.1)
                    $(.with_message($label))?
            ]))?
            $(.with_notes(vec![$note.to_string()]))?
    };
    ($msg:expr $(, $span:expr $(,)?)?) => {{
        ::codespan_reporting::diagnostic::Diagnostic::error()
            .with_message($msg.to_string())
            $(.with_labels(vec![::codespan_reporting::diagnostic::Label::primary($span.2, $span.0..$span.1)]))?
    }};
}

pub trait DiagnosticExt: Sized {
    fn with_label(self, label: Label<usize>) -> Self;
}

impl DiagnosticExt for Diagnostic<usize> {
    fn with_label(self, label: Label<usize>) -> Self {
        self.with_labels(vec![label])
    }
}

pub trait ToDiagnostic: std::fmt::Debug {
    fn to_diagnostic(&self) -> Diagnostic<usize>;
    fn to_message(&self) -> String {
        self.to_diagnostic().message
    }
}

impl ToDiagnostic for Diagnostic<usize> {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        self.clone()
    }
}
impl ToDiagnostic for AstError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            AstError::UnknownToplevel(span, name) => gen_diagnostic!(self, span),
            AstError::MissingNode(span) => gen_diagnostic! {
                msg = "Expected another element",
                label = span => "Expected another element here",
            },

            AstError::WrongExprType(span, expected, actual) => gen_diagnostic! {
                msg = "Wrong type of expression",
                label = span => format!("Expected a `{}` here", expected),
                note = format!("Expected: {}\nGot: {}", expected, actual),
            },
            AstError::NotAValue(span, actual) => gen_diagnostic! {
                msg = format!("Expected value, but got `{}`", actual),
                label = span => "Expected some value here",
                note = format!("Got: {}", actual),
            },

            AstError::ParseError { file_id, source } => lalrpop_error_to_diagnostic(source, *file_id),
            AstError::MismatchedElementName(span, expected, got) => gen_diagnostic! {
                msg = format!("Expected element `{}`, but found `{}`", expected, got),
                label = span => format!("Expected `{}` here", expected),
                note = format!("Expected: {}\nGot: {}", expected, got),
            },
            AstError::ErrorContext { label_span, context, main_err } => {
                main_err.to_diagnostic().with_label(span_to_secondary_label(*label_span).with_message(context))
            }

            AstError::ConversionError(source) => source.to_diagnostic(),
            AstError::Other(span, source) => gen_diagnostic!(source, span),
            AstError::AttrError(source) => source.to_diagnostic(),
            AstError::IncludedFileNotFound(include) => gen_diagnostic!(
                msg = format!("Included file `{}` not found", include.path),
                label = include.path_span => "Included here",
            ),

            AstError::TooManyNodes(extra_nodes_span, expected) => gen_diagnostic! {
                msg = self,
                label = extra_nodes_span => "these elements must not be here",
                note = "Consider wrapping the elements in some container element",
            },
            AstError::ErrorNote(note, source) => source.to_diagnostic().with_notes(vec![note.to_string()]),
            AstError::ValidationError(source) => source.to_diagnostic(),
        }
    }
}

impl ToDiagnostic for parse_error::ParseError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            parse_error::ParseError::SimplExpr(error) => error.to_diagnostic(),
            parse_error::ParseError::LexicalError(span) => generate_lexical_error_diagnostic(*span),
        }
    }
}

impl ToDiagnostic for AttrError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            AttrError::MissingRequiredAttr(span, attr_name) => {
                gen_diagnostic!(format!("Missing attribute `{}`", attr_name), span)
            }
            AttrError::EvaluationError(span, source) => source.to_diagnostic(),
            AttrError::Other(span, source) => gen_diagnostic!(source, span),
        }
    }
}

impl ToDiagnostic for ValidationError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            ValidationError::UnknownWidget(span, name) => gen_diagnostic! {
                msg = self,
                label = span => "Used here",
            },
            ValidationError::MissingAttr { widget_name, arg_name, arg_list_span, use_span } => {
                let mut diag =
                    gen_diagnostic!(self).with_label(span_to_secondary_label(*use_span).with_message("Argument missing here"));
                if let Some(arg_list_span) = arg_list_span {
                    diag = diag.with_label(span_to_secondary_label(*arg_list_span).with_message("But is required here"));
                }
                diag
            }
        }
    }
}

fn lalrpop_error_to_diagnostic<T: std::fmt::Display, E: Spanned + ToDiagnostic>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    file_id: usize,
) -> Diagnostic<usize> {
    use lalrpop_util::ParseError::*;
    match error {
        InvalidToken { location } => gen_diagnostic!("Invalid token", Span::point(*location, file_id)),
        UnrecognizedEOF { location, expected } => gen_diagnostic! {
            msg = "Input ended unexpectedly. Check if you have any unclosed delimiters",
            label = Span::point(*location, file_id),
        },
        UnrecognizedToken { token, expected } => gen_diagnostic! {
            msg = format!("Unexpected token `{}` encountered", token.1),
            label = Span(token.0, token.2, file_id) => "Token unexpected",
        },
        ExtraToken { token } => gen_diagnostic!(format!("Extra token encountered: `{}`", token.1)),
        User { error } => error.to_diagnostic(),
    }
}

impl ToDiagnostic for simplexpr::error::Error {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        use simplexpr::error::Error::*;
        match self {
            ParseError { source, file_id } => lalrpop_error_to_diagnostic(source, *file_id),
            ConversionError(error) => error.to_diagnostic(),
            Eval(error) => error.to_diagnostic(),
            Other(error) => gen_diagnostic!(error),
            Spanned(span, error) => gen_diagnostic!(error, span),
        }
    }
}

impl ToDiagnostic for simplexpr::parser::lexer::LexicalError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        generate_lexical_error_diagnostic(self.span())
    }
}

impl ToDiagnostic for simplexpr::eval::EvalError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        use simplexpr::eval::EvalError::*;
        match self {
            UnresolvedVariable(name) | UnknownVariable(name) | NoVariablesAllowed(name) => gen_diagnostic! {
                msg = self,
                note = format!("If you meant to use the literal value \"{}\", surround the value in quotes", name)
            },
            Spanned(span, error) => error.as_ref().to_diagnostic().with_label(span_to_primary_label(*span)),
            _ => gen_diagnostic!(self, self.span()),
        }
    }
}

impl ToDiagnostic for dynval::ConversionError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        let diag = gen_diagnostic! {
            msg = self,
            label = self.value.span() => format!("`{}` is not of type `{}`", self.value, self.target_type),
        };
        diag.with_notes(self.source.as_ref().map(|x| vec![format!("{}", x)]).unwrap_or_default())
    }
}

fn generate_lexical_error_diagnostic(span: Span) -> Diagnostic<usize> {
    gen_diagnostic! {
        msg = "Invalid token",
        label = span => "Invalid token"
    }
}
