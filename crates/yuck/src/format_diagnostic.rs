use codespan_reporting::{diagnostic, files};
use config::TOP_LEVEL_DEFINITION_NAMES;
use itertools::Itertools;
use simplexpr::dynval;

use diagnostic::*;

use crate::{
    config::{attributes::AttrError, config, validate::ValidationError},
    error::{get_parse_error_span, AstError, FormFormatError},
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
            AstError::UnknownToplevel(span, name) => gen_diagnostic! {
                msg = self,
                label = span,
                note = format!("Must be one of: {}", TOP_LEVEL_DEFINITION_NAMES.iter().join(", "))
            },
            AstError::MissingNode(span) => gen_diagnostic! {
                msg = "Expected another element",
                label = span => "Expected another element here",
            },
            AstError::WrongExprType(span, expected, actual) => gen_diagnostic! {
                msg = "Wrong type of expression",
                label = span => format!("Expected a `{}` here", expected),
                note = format!("Expected: {}\n     Got: {}", expected, actual),
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
                note = format!("Expected: {}\n     Got: {}", expected, got),
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
            AstError::DanglingKeyword(span, keyword) => gen_diagnostic! {
                msg = self,
                label = span => "No value provided for this",
            },
            AstError::ErrorNote(note, source) => source.to_diagnostic().with_notes(vec![note.to_string()]),
            AstError::ValidationError(source) => source.to_diagnostic(),
            AstError::NoMoreElementsExpected(span) => gen_diagnostic!(self, span),
            AstError::SimplExpr(source) => source.to_diagnostic(),
            AstError::FormFormatError(error) => error.to_diagnostic(),
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
        }
    }
}

fn variable_deprecation_note(var_name: String) -> Option<String> {
    (var_name == "EWW_CPU_USAGE")
        .then(|| "Note: EWW_CPU_USAGE has recently been removed, and has now been renamed to EWW_CPU".to_string())
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
            Spanned(span, error) => error.to_diagnostic().with_label(span_to_primary_label(*span)),
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
            NoVariablesAllowed(name) => gen_diagnostic!(self),
            UnknownVariable(name, similar) => {
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
        diag.with_notes(self.source.as_ref().map(|x| vec![x.to_string()]).unwrap_or_default())
    }
}

fn generate_lexical_error_diagnostic(span: Span) -> Diagnostic<usize> {
    gen_diagnostic! {
        msg = "Invalid token",
        label = span => "Invalid token"
    }
}

impl ToDiagnostic for FormFormatError {
    fn to_diagnostic(&self) -> diagnostic::Diagnostic<usize> {
        match self {
            FormFormatError::WidgetDefArglistMissing(span) => gen_diagnostic! {
                msg = self,
                label = span => "Insert the argument list (e.g.: `[]`) here",
                note = "This list will in the future need to declare all the non-global variables / attributes used in this widget.\n\
                        This is not yet neccessary, but is still considered good style.",
            },
            FormFormatError::WidgetDefMultipleChildren(span) => gen_diagnostic! {
                msg = self,
                label = span => "Found more than one child element here.",
                note = "A widget-definition may only contain one child element.\n\
                        To include multiple elements, wrap these elements in a single container widget such as `box`.\n\
                        This is necessary as eww can't know how you want these elements to be layed out otherwise."
            },
            FormFormatError::ExpectedInInForLoop(span, got) => gen_diagnostic! {
                msg = self,
                label = span,
            },
        }
    }
}
