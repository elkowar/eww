use codespan_reporting::diagnostic;
use itertools::Itertools;
use simplexpr::dynval;

use diagnostic::*;

use crate::config::{attributes::AttrError, validate::ValidationError};

use super::parser::parse_error;
use eww_shared_util::{Span, Spanned};

pub fn span_to_primary_label(span: Span) -> Label<usize> {
    Label::primary(span.2, span.0..span.1)
}
pub fn span_to_secondary_label(span: Span) -> Label<usize> {
    Label::secondary(span.2, span.0..span.1)
}

/// Generate a nicely formatted diagnostic
/// ```rs
/// gen_diagnostic! {
///     kind = Severity::Error,
///     msg = format!("Expected value, but got `{}`", actual),
///     label = span => "Expected some value here",
///     note = format!("Got: {}", actual),
/// }
/// ```
#[macro_export]
macro_rules! gen_diagnostic {
    ( $(kind = $kind:expr,)?
      $(msg = $msg:expr)?
      $(, label = $span:expr $(=> $label:expr)?)?
      $(, note = $note:expr)? $(,)?
    ) => {
        ::codespan_reporting::diagnostic::Diagnostic::new(gen_diagnostic! {
            @macro_fallback $({$kind})? {::codespan_reporting::diagnostic::Severity::Error}
        })
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


    (@macro_fallback { $value:expr } { $fallback:expr }) => {
        $value
    };
    (@macro_fallback { $fallback:expr }) => {
        $fallback
    };
}

pub trait DiagnosticExt: Sized {
    fn with_label(self, label: Label<usize>) -> Self;
    fn with_note(self, note: String) -> Self;
}

impl DiagnosticExt for Diagnostic<usize> {
    fn with_label(self, label: Label<usize>) -> Self {
        self.with_labels(vec![label])
    }

    fn with_note(self, note: String) -> Self {
        self.with_notes(vec![note])
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

impl ToDiagnostic for parse_error::ParseError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            parse_error::ParseError::SimplExpr(source) => lalrpop_error_to_diagnostic(&source.source, source.file_id),
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
            AttrError::EvaluationError(_span, source) => source.to_diagnostic(),
            AttrError::Other(span, source) => gen_diagnostic!(source, span),
        }
    }
}

impl ToDiagnostic for ValidationError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        match self {
            ValidationError::MissingAttr { widget_name, arg_name, arg_list_span, use_span } => {
                let mut diag = Diagnostic::error()
                    .with_message(self.to_string())
                    .with_label(span_to_secondary_label(*use_span).with_message("Argument missing here"))
                    .with_notes(vec![format!(
                        "Hint: pass the attribute like so: `({} :{} your-value ...`",
                        widget_name, arg_name
                    )]);
                if let Some(arg_list_span) = arg_list_span {
                    diag = diag.with_label(span_to_secondary_label(*arg_list_span).with_message("But is required here"));
                }
                diag
            }
            ValidationError::UnknownVariable { span, name, in_definition } => {
                let diag = gen_diagnostic! {
                    msg = self,
                    label = span => "Used here",
                    note = if *in_definition {
                        "Hint: Either define it as a global variable, or add it to the argument-list of your `defwidget` and pass it as an argument"
                    } else {
                        "Hint: Define it as a global variable"
                    }
                };

                let mut extra_notes =
                    vec![format!("Hint: If you meant to use the literal value \"{}\", surround the value in quotes", name)];

                if let Some(deprecation_note) = variable_deprecation_note(name.to_string()) {
                    extra_notes.push(deprecation_note)
                };

                diag.with_notes(extra_notes)
            }
            ValidationError::AccidentalBuiltinOverride(span, _widget_name) => gen_diagnostic! {
                msg = self,
                label = span => "Defined here",
                note = "Hint: Give your widget a different name. You could call it \"John\" for example. That's a cool name."
            },
        }
    }
}

fn variable_deprecation_note(var_name: String) -> Option<String> {
    (var_name == "EWW_CPU_USAGE")
        .then(|| "Note: EWW_CPU_USAGE has recently been removed, and has now been renamed to EWW_CPU".to_string())
}

pub fn lalrpop_error_to_diagnostic<T: std::fmt::Display, E: Spanned + ToDiagnostic>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    file_id: usize,
) -> Diagnostic<usize> {
    use lalrpop_util::ParseError::*;
    match error {
        InvalidToken { location } => gen_diagnostic!("Invalid token", Span::point(*location, file_id)),
        UnrecognizedEof { location, expected: _ } => gen_diagnostic! {
            msg = "Input ended unexpectedly. Check if you have any unclosed delimiters",
            label = Span::point(*location, file_id),
        },
        UnrecognizedToken { token, expected: _ } => gen_diagnostic! {
            msg = format!("Unexpected token `{}` encountered", token.1),
            label = Span(token.0, token.2, file_id) => "Token unexpected",
        },
        ExtraToken { token } => gen_diagnostic!(format!("Extra token encountered: `{}`", token.1)),
        User { error } => error.to_diagnostic(),
    }
}

impl ToDiagnostic for simplexpr::parser::lexer::LexicalError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        generate_lexical_error_diagnostic(self.span())
    }
}

impl ToDiagnostic for simplexpr::eval::EvalError {
    fn to_diagnostic(&self) -> Diagnostic<usize> {
        use simplexpr::eval::EvalError;
        match self {
            EvalError::NoVariablesAllowed(_name) => gen_diagnostic!(self),
            EvalError::UnknownVariable(name, similar) => {
                let mut notes = Vec::new();
                if similar.len() == 1 {
                    notes.push(format!("Did you mean `{}`?", similar.first().unwrap()))
                } else if similar.len() > 1 {
                    notes.push(format!("Did you mean one of: {}?", similar.iter().map(|x| format!("`{}`", x)).join(", ")))
                }
                // TODO the note here is confusing when it's an unknown variable being used _within_ a string literal / simplexpr
                // it only really makes sense on top-level symbols
                notes.push(format!("Hint: If you meant to use the literal value \"{}\", surround the value in quotes", name));
                gen_diagnostic!(self).with_notes(notes)
            }
            EvalError::Spanned(span, err) => {
                if let EvalError::JaqParseError(err) = err.as_ref() {
                    if let Some(ref err) = err.as_ref().0 {
                        let span = span.new_relative(err.span().start, err.span().end).shifted(1);
                        let mut diag = gen_diagnostic!(self, span);

                        if let Some(label) = err.label() {
                            diag = diag.with_label(span_to_secondary_label(span).with_message(label));
                        }

                        let expected: Vec<_> = err.expected().filter_map(|x| x.clone()).sorted().collect();
                        if !expected.is_empty() {
                            let label = format!("Expected one of {} here", expected.join(", "));
                            diag = diag.with_label(span_to_primary_label(span).with_message(label));
                        }
                        return diag;
                    }
                }
                return err.as_ref().to_diagnostic().with_label(span_to_primary_label(*span));
            }
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
        diag.with_notes(self.source.as_ref().map(|x| vec![x.to_string()]).unwrap_or_default())
    }
}

fn generate_lexical_error_diagnostic(span: Span) -> Diagnostic<usize> {
    gen_diagnostic! {
        msg = "Invalid token",
        label = span => "Invalid token"
    }
}
