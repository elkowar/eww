use std::sync::{Arc, Mutex};

use codespan_reporting::diagnostic::Diagnostic;
use eww_shared_util::DUMMY_SPAN;
use simplexpr::eval::EvalError;
use yuck::{
    config::file_provider::FsYuckFiles,
    error::AstError,
    format_diagnostic::{eval_error_to_diagnostic, ToDiagnostic},
};

use crate::error::DiagError;

lazy_static::lazy_static! {
    pub static ref ERROR_HANDLING_CTX: Arc<Mutex<FsYuckFiles>> = Arc::new(Mutex::new(FsYuckFiles::new()));
}

pub fn clear_files() {
    *ERROR_HANDLING_CTX.lock().unwrap() = FsYuckFiles::new();
}

pub fn print_error(err: &anyhow::Error) {
    if let Some(err) = err.downcast_ref::<DiagError>() {
        eprintln!("{:?}\n{}", err, stringify_diagnostic(&err.diag));
    } else if let Some(err) = err.downcast_ref::<AstError>() {
        eprintln!("{:?}\n{}", err, stringify_diagnostic(&err.to_diagnostic()));
    } else if let Some(err) = err.downcast_ref::<EvalError>() {
        eprintln!("{:?}\n{}", err, stringify_diagnostic(&eval_error_to_diagnostic(err, err.span().unwrap_or(DUMMY_SPAN))));
    } else {
        log::error!("{:?}", err);
    }
}

pub fn format_error(err: &anyhow::Error) -> String {
    match err.downcast_ref::<AstError>() {
        Some(err) => stringify_diagnostic(&err.to_diagnostic()),
        None => format!("{:?}", err),
    }
}

pub fn stringify_diagnostic(diagnostic: &Diagnostic<usize>) -> String {
    use codespan_reporting::term;
    let config = term::Config::default();
    let mut buf = Vec::new();
    let mut writer = term::termcolor::Ansi::new(&mut buf);
    let files = ERROR_HANDLING_CTX.lock().unwrap();
    term::emit(&mut writer, &config, &*files, &diagnostic).unwrap();
    String::from_utf8(buf).unwrap()
}
