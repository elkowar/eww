use std::sync::{Arc, Mutex};

use yuck::{config::file_provider::FsYuckFiles, error::AstError, format_diagnostic::ToDiagnostic};

lazy_static::lazy_static! {
    pub static ref ERROR_HANDLING_CTX: Arc<Mutex<FsYuckFiles>> = Arc::new(Mutex::new(FsYuckFiles::new()));
}

pub fn clear_files() {
    *ERROR_HANDLING_CTX.lock().unwrap() = FsYuckFiles::new();
}

pub fn print_error(err: anyhow::Error) {
    match err.downcast_ref::<AstError>() {
        Some(err) => {
            print_ast_error(err);
        }
        None => {
            log::error!("{:?}", err);
        }
    }
}

pub fn print_ast_error(err: &AstError) {
    let diag = err.to_diagnostic();
    use codespan_reporting::term;
    let config = term::Config::default();
    let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
    let files = ERROR_HANDLING_CTX.lock().unwrap();
    term::emit(&mut writer, &config, &*files, &diag).unwrap();
}

pub fn format_error(err: anyhow::Error) -> String {
    match err.downcast_ref::<AstError>() {
        Some(err) => format_ast_error(err),
        None => format!("{:?}", err),
    }
}

pub fn format_ast_error(err: &AstError) -> String {
    let diag = err.to_diagnostic();
    use codespan_reporting::term;
    let config = term::Config::default();
    // let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
    let mut buf = Vec::new();
    let mut writer = term::termcolor::Ansi::new(&mut buf);
    let files = ERROR_HANDLING_CTX.lock().unwrap();
    term::emit(&mut writer, &config, &*files, &diag).unwrap();
    String::from_utf8(buf).unwrap()
}
