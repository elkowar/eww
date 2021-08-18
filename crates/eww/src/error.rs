use codespan_reporting::diagnostic::Diagnostic;

/// An error that contains a [Diagnostic] for ad-hoc creation of diagnostics.
#[derive(Debug)]
pub struct DiagError {
    pub diag: Diagnostic<usize>,
}

impl DiagError {
    pub fn new(diag: Diagnostic<usize>) -> Self {
        Self { diag }
    }
}

impl std::error::Error for DiagError {}
impl std::fmt::Display for DiagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diag.message)
    }
}
