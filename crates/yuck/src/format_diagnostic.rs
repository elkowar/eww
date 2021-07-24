use codespan_reporting::{diagnostic, files};
use simplexpr::dynval;

use diagnostic::*;

use crate::{
    config::{attributes::AttrError, validate::ValidationError},
    error::{get_parse_error_span, AstError},
};

use super::parser::parse_error;
use eww_shared_util::{AttrName, Span, VarName};

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
        // TODO this if let should be unnecessary
        if let AstError::ValidationError(error) = self {
            error.to_diagnostic()
        } else {
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

                AstError::ParseError { file_id, source } => lalrpop_error_to_diagnostic(source, *file_id, |error| error.to_diagnostic()),
                AstError::MismatchedElementName(span, expected, got) => gen_diagnostic! {
                    msg = format!("Expected element `{}`, but found `{}`", expected, got),
                    label = span => format!("Expected `{}` here", expected),
                    note = format!("Expected: {}\nGot: {}", expected, got),
                },
                AstError::ErrorContext { label_span, context, main_err } => {
                    main_err.to_diagnostic().with_opt_label(Some(span_to_secondary_label(*label_span).with_message(context)))
                }

                AstError::ConversionError(source) => source.to_diagnostic(),
                AstError::Other(Some(span), source) => gen_diagnostic!(source, span),
                AstError::Other(None, source) => gen_diagnostic!(source),
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
}

impl ToDiagnostic for parse_error::ParseError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            parse_error::ParseError::SimplExpr(error) => error.to_diagnostic(),
            parse_error::ParseError::LexicalError(span) => lexical_error_diagnostic(*span),
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
            ValidationError::MissingAttr { widget_name, arg_name, arg_list_span, use_span } => gen_diagnostic!(self)
                .with_opt_label(Some(span_to_secondary_label(*use_span).with_message("Argument missing here")))
                .with_opt_label(arg_list_span.map(|s| span_to_secondary_label(s).with_message("but is required here"))),
        }
    }
}

fn lalrpop_error_to_diagnostic<T: std::fmt::Display, E: std::fmt::Display>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    file_id: usize,
    handle_user_error: impl FnOnce(&E) -> Diagnostic<usize>,
) -> Diagnostic<usize> {
    use lalrpop_util::ParseError::*;
    // None is okay here, as the case that would be affected by it (User { error }) is manually handled here anyways
    let span = get_parse_error_span(file_id, error, |e| None);
    let res: Option<_> = try {
        match error {
            InvalidToken { location } => gen_diagnostic!("Invalid token", span?),
            UnrecognizedEOF { location, expected } => gen_diagnostic! {
                "Input ended unexpectedly. Check if you have any unclosed delimiters",
                span?
            },
            UnrecognizedToken { token, expected } => gen_diagnostic! {
                msg = format!("Unexpected token `{}` encountered", token.1),
                label = span? => "Token unexpected",
            },
            ExtraToken { token } => gen_diagnostic!(format!("Extra token encountered: `{}`", token.1)),
            User { error } => handle_user_error(error),
        }
    };
    res.unwrap_or_else(|| gen_diagnostic!(error))
}

impl ToDiagnostic for simplexpr::error::Error {
    // TODO this needs a lot of improvement
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        use simplexpr::error::Error::*;
        let res: Option<_> = try {
            match self {
                ParseError { source, file_id } => {
                    let span = get_parse_error_span(*file_id, source, |e| Some(Span(e.0, e.1, *file_id)))?;
                    lalrpop_error_to_diagnostic(source, *file_id, move |error| lexical_error_diagnostic(span))
                }
                ConversionError(error) => error.to_diagnostic(),
                Eval(error) => error.to_diagnostic(),
                Other(error) => gen_diagnostic!(error),
                Spanned(span, error) => gen_diagnostic!(error, span),
            }
        };
        res.unwrap_or_else(|| gen_diagnostic!(self))
    }
}

impl ToDiagnostic for simplexpr::eval::EvalError {
    // TODO this needs a lot of improvement
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self.span() {
            Some(span) => gen_diagnostic!(self, span),
            None => gen_diagnostic!(self),
        }
    }
}

impl ToDiagnostic for dynval::ConversionError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        let diag = match self.span() {
            Some(span) => gen_diagnostic! {
                msg = self,
                label = span => format!("`{}` is not of type `{}`", self.value, self.target_type),
            },
            None => gen_diagnostic!(self),
        };
        diag.with_notes(self.source.as_ref().map(|x| vec![format!("{}", x)]).unwrap_or_default())
    }
}

/// Generate a simple diagnostic indicating a lexical error
fn lexical_error_diagnostic(span: Span) -> Diagnostic<usize> {
    gen_diagnostic! {
        msg = "Invalid token",
        label = span => "Invalid token"
    }
}
