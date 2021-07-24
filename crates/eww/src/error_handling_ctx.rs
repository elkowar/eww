use std::sync::{Arc, Mutex};

use codespan_reporting::{diagnostic::Diagnostic, term, term::Chars};
use eww_shared_util::DUMMY_SPAN;
use simplexpr::eval::EvalError;
use yuck::{
    config::file_provider::YuckFiles,
    error::AstError,
    format_diagnostic::{eval_error_to_diagnostic, ToDiagnostic},
    gen_diagnostic,
};

use crate::error::DiagError;

lazy_static::lazy_static! {
    pub static ref ERROR_HANDLING_CTX: Arc<Mutex<YuckFiles>> = Arc::new(Mutex::new(YuckFiles::new()));
}

pub fn clear_files() {
    *ERROR_HANDLING_CTX.lock().unwrap() = YuckFiles::new();
}

pub fn anyhow_err_to_diagnostic(err: &anyhow::Error) -> Diagnostic<usize> {
    if let Some(err) = err.downcast_ref::<DiagError>() {
        err.diag.clone()
    } else if let Some(err) = err.downcast_ref::<AstError>() {
        err.to_diagnostic()
    } else if let Some(err) = err.downcast_ref::<EvalError>() {
        eval_error_to_diagnostic(err, err.span().unwrap_or(DUMMY_SPAN))
    } else {
        gen_diagnostic!(err)
    }
}

pub fn print_error(err: anyhow::Error) {
    match stringify_diagnostic(&anyhow_err_to_diagnostic(&err)) {
        Ok(diag) => {
            eprintln!("{:?}\n{}", err, diag);
        }
        Err(_) => {
            log::error!("{:?}", err);
        }
    }
}

pub fn format_error(err: &anyhow::Error) -> String {
    for err in err.chain() {
        format!("chain: {}", err);
    }
    match err.downcast_ref::<AstError>() {
        Some(err) => stringify_diagnostic(&err.to_diagnostic()).unwrap_or_else(|_| format!("{:?}", err)),
        None => format!("{:?}", err),
    }
}

pub fn stringify_diagnostic(diagnostic: &codespan_reporting::diagnostic::Diagnostic<usize>) -> anyhow::Result<String> {
    let mut config = term::Config::default();
    let mut chars = Chars::box_drawing();
    chars.single_primary_caret = '─';
    config.chars = chars;
    let mut buf = Vec::new();
    let mut writer = term::termcolor::Ansi::new(&mut buf);
    let files = ERROR_HANDLING_CTX.lock().unwrap();
    term::emit(&mut writer, &config, &*files, &diagnostic)?;
    Ok(String::from_utf8(buf)?)
}
