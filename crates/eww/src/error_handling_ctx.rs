//! Disgusting global state.
//! I hate this, but [buffet](https://github.com/buffet) told me that this is what I should do for peak maintainability!

use std::sync::{Arc, RwLock};

use codespan_reporting::{
    diagnostic::Diagnostic,
    term::{self, Chars},
};
use eww_shared_util::Span;
use once_cell::sync::Lazy;
use simplexpr::{dynval::ConversionError, eval::EvalError};
use yuck::{
    config::{file_provider::YuckFiles, validate::ValidationError},
    error::AstError,
    format_diagnostic::ToDiagnostic,
};

use crate::error::DiagError;

pub static YUCK_FILES: Lazy<Arc<RwLock<YuckFiles>>> = Lazy::new(|| Arc::new(RwLock::new(YuckFiles::new())));

pub fn clear_files() {
    *YUCK_FILES.write().unwrap() = YuckFiles::new();
}

pub fn print_error(err: anyhow::Error) {
    match anyhow_err_to_diagnostic(&err) {
        Some(diag) => match stringify_diagnostic(diag) {
            Ok(diag) => {
                eprintln!("{}", diag);
            }
            Err(_) => {
                log::error!("{:?}", err);
            }
        },
        None => {
            log::error!("{:?}", err);
        }
    }
}

pub fn format_error(err: &anyhow::Error) -> String {
    for err in err.chain() {
        format!("chain: {}", err);
    }
    anyhow_err_to_diagnostic(err).and_then(|diag| stringify_diagnostic(diag).ok()).unwrap_or_else(|| format!("{:?}", err))
}

pub fn anyhow_err_to_diagnostic(err: &anyhow::Error) -> Option<Diagnostic<usize>> {
    if let Some(err) = err.downcast_ref::<DiagError>() {
        Some(err.diag.clone())
    } else if let Some(err) = err.downcast_ref::<AstError>() {
        Some(err.to_diagnostic())
    } else if let Some(err) = err.downcast_ref::<ConversionError>() {
        Some(err.to_diagnostic())
    } else if let Some(err) = err.downcast_ref::<ValidationError>() {
        Some(err.to_diagnostic())
    } else if let Some(err) = err.downcast_ref::<EvalError>() {
        Some(err.to_diagnostic())
    } else {
        None
    }
}

pub fn stringify_diagnostic(mut diagnostic: codespan_reporting::diagnostic::Diagnostic<usize>) -> anyhow::Result<String> {
    diagnostic.labels.drain_filter(|label| Span(label.range.start, label.range.end, label.file_id).is_dummy());

    let mut config = term::Config::default();
    let mut chars = Chars::box_drawing();
    chars.single_primary_caret = '─';
    config.chars = chars;
    config.chars.note_bullet = '→';
    let mut buf = Vec::new();
    let mut writer = term::termcolor::Ansi::new(&mut buf);
    let files = YUCK_FILES.read().unwrap();
    term::emit(&mut writer, &config, &*files, &diagnostic)?;
    Ok(String::from_utf8(buf)?)
}
