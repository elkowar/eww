use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use itertools::Itertools;

use super::{
    file_provider::{FilesError, YuckFileProvider},
    script_var_definition::ScriptVarDefinition,
    var_definition::VarDefinition,
    widget_definition::WidgetDefinition,
    window_definition::WindowDefinition,
};
use crate::{
    config::script_var_definition::{ListenScriptVar, PollScriptVar},
    error::{DiagError, DiagResult},
    gen_diagnostic,
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{Span, Spanned, VarName};

static TOP_LEVEL_DEFINITION_NAMES: &[&str] = &[
    WidgetDefinition::ELEMENT_NAME,
    WindowDefinition::ELEMENT_NAME,
    VarDefinition::ELEMENT_NAME,
    ListenScriptVar::ELEMENT_NAME,
    PollScriptVar::ELEMENT_NAME,
    Include::ELEMENT_NAME,
];

/// Defines common ways definitions may be called instead of their official names.
///
/// E.g: ~~`import`~~ ➡️ `include`.
///
/// This list will be used to generate a hint in the error diagnostic suggesting the use of the correct definition.
///
/// **Note:** This is not meant to contain a list of typos for [`TOP_LEVEL_DEFINITION_NAMES`], instead it contains a
/// list of correctly spelled strings that may be used because they exist in other languages.
static TOP_LEVEL_COMMON_DEFINITION_ERRORS: &[CommonDefinitionError] = {
    use CommonDefinitionError as E; // Makes the lines below shorter
    &[E { wrong: "import", correct: Include::ELEMENT_NAME }]
};

/// Used to map commonly confused definitions to their correct naming
struct CommonDefinitionError<'a> {
    wrong: &'a str,
    correct: &'a str,
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct Include {
    pub path: String,
    pub path_span: Span,
}

impl FromAstElementContent for Include {
    const ELEMENT_NAME: &'static str = "include";

    fn from_tail<I: Iterator<Item = Ast>>(_span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let (path_span, path) = iter.expect_literal()?;
        iter.expect_done()?;
        Ok(Include { path: path.to_string(), path_span })
    }
}

pub enum TopLevel {
    Include(Include),
    VarDefinition(VarDefinition),
    ScriptVarDefinition(ScriptVarDefinition),
    WidgetDefinition(WidgetDefinition),
    WindowDefinition(WindowDefinition),
}

impl FromAst for TopLevel {
    fn from_ast(e: Ast) -> DiagResult<Self> {
        let span = e.span();
        let mut iter = e.try_ast_iter()?;
        let (sym_span, element_name) = iter.expect_symbol()?;
        Ok(match element_name.as_str() {
            x if x == Include::ELEMENT_NAME => Self::Include(Include::from_tail(span, iter)?),
            x if x == WidgetDefinition::ELEMENT_NAME => Self::WidgetDefinition(WidgetDefinition::from_tail(span, iter)?),
            x if x == VarDefinition::ELEMENT_NAME => Self::VarDefinition(VarDefinition::from_tail(span, iter)?),
            x if x == PollScriptVar::ELEMENT_NAME => {
                Self::ScriptVarDefinition(ScriptVarDefinition::Poll(PollScriptVar::from_tail(span, iter)?))
            }
            x if x == ListenScriptVar::ELEMENT_NAME => {
                Self::ScriptVarDefinition(ScriptVarDefinition::Listen(ListenScriptVar::from_tail(span, iter)?))
            }
            x if x == WindowDefinition::ELEMENT_NAME => Self::WindowDefinition(WindowDefinition::from_tail(span, iter)?),
            x => {
                for common_error in TOP_LEVEL_COMMON_DEFINITION_ERRORS {
                    if x == common_error.wrong {
                        return Err(DiagError(gen_diagnostic! {
                            msg = format!("Unknown toplevel declaration `{x}`"),
                            label = sym_span,
                            note = format!("help: Perhaps you've meant `{}`?", common_error.correct),
                            note = format!("Must be one of: {}", TOP_LEVEL_DEFINITION_NAMES.iter().join(", ")),
                        }));
                    }
                }

                return Err(DiagError(gen_diagnostic! {
                    msg = format!("Unknown toplevel declaration `{x}`"),
                    label = sym_span,
                    note = format!("Must be one of: {}", TOP_LEVEL_DEFINITION_NAMES.iter().join(", ")),
                }));
            }
        })
    }
}

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct Config {
    pub widget_definitions: HashMap<String, WidgetDefinition>,
    pub window_definitions: HashMap<String, WindowDefinition>,
    pub var_definitions: HashMap<VarName, VarDefinition>,
    pub script_vars: HashMap<VarName, ScriptVarDefinition>,
}

impl Config {
    fn append_toplevel(
        &mut self,
        files: &mut impl YuckFileProvider,
        toplevel: TopLevel,
        path: impl AsRef<Path>,
    ) -> DiagResult<()> {
        match toplevel {
            TopLevel::VarDefinition(x) => {
                if self.var_definitions.contains_key(&x.name) || self.script_vars.contains_key(&x.name) {
                    return Err(DiagError(gen_diagnostic! {
                        msg = format!("Variable {} defined twice", x.name),
                        label = x.span => "defined again here",
                    }));
                } else {
                    self.var_definitions.insert(x.name.clone(), x);
                }
            }
            TopLevel::ScriptVarDefinition(x) => {
                if self.var_definitions.contains_key(x.name()) || self.script_vars.contains_key(x.name()) {
                    return Err(DiagError(gen_diagnostic! {
                        msg = format!("Variable {} defined twice", x.name()),
                        label = x.name_span() => "defined again here",
                    }));
                } else {
                    self.script_vars.insert(x.name().clone(), x);
                }
            }
            TopLevel::WidgetDefinition(x) => {
                self.widget_definitions.insert(x.name.clone(), x);
            }
            TopLevel::WindowDefinition(x) => {
                self.window_definitions.insert(x.name.clone(), x);
            }
            TopLevel::Include(include) => {
                // Resolve the potentially relative path to it's target
                let mut include_path = PathBuf::from(&include.path);
                if include_path.starts_with("./") || include_path.starts_with("../") {
                    // Allows relative paths to go beyond the config directory.
                    //
                    // Should not panic unless the file we just read doesn't exist anymore or the path points to a file
                    // that is indexed as a directory (`eww.yuck/test.txt`, where `eww.yuck` is a file).
                    //
                    // Since both cases are extremly rare to happen there is no point in making overly verbose
                    // diagnostics.
                    let canonical_path =
                        path.as_ref().canonicalize().expect("Failed to canonicalize `{path}` due to a filesystem error.");

                    include_path = util::resolve_relative_file(canonical_path, include_path);
                }

                let (_, toplevels) = files.load_yuck_file(include_path.clone()).map_err(|err| match err {
                    FilesError::IoError(_) => DiagError(gen_diagnostic! {
                    msg = format!("Included file `{}` not found", include.path),
                    label = include.path_span => "Included here",
                    note = format!("Hint: Resolved to `{}`", include_path.to_string_lossy()),
                    }),
                    FilesError::DiagError(x) => x,
                })?;

                for element in toplevels {
                    self.append_toplevel(files, TopLevel::from_ast(element)?, &include_path)?;
                }
            }
        }
        Ok(())
    }

    pub fn generate(files: &mut impl YuckFileProvider, elements: Vec<Ast>, path: impl AsRef<Path>) -> DiagResult<Self> {
        let mut config = Self {
            widget_definitions: HashMap::new(),
            window_definitions: HashMap::new(),
            var_definitions: HashMap::new(),
            script_vars: HashMap::new(),
        };
        for element in elements {
            config.append_toplevel(files, TopLevel::from_ast(element)?, &path)?;
        }
        Ok(config)
    }

    pub fn generate_from_main_file(files: &mut impl YuckFileProvider, path: impl AsRef<Path>) -> DiagResult<Self> {
        let (_span, top_levels) = files.load_yuck_file(path.as_ref().to_path_buf()).map_err(|err| match err {
            FilesError::IoError(err) => DiagError(gen_diagnostic!(err)),
            FilesError::DiagError(x) => x,
        })?;
        Self::generate(files, top_levels, path)
    }
}

//‌‌/‌ Contains code that makes assumptions about how it is used. Thus it is only avaliable to this module.
mod util {
    use std::path::{Path, PathBuf};

    /// Takes two paths and retuns location of `path_offset` relative to `base_file`.
    /// Both cases of `base_file` being a file or a directory are handled.
    ///
    /// The resulting location could be invalid, a directory or a file.
    pub fn resolve_relative_file(base_file: impl AsRef<Path>, path_offset: impl AsRef<Path>) -> PathBuf {
        let base_file = base_file.as_ref();
        let path_offset = path_offset.as_ref();

        // if "" then path_offset is already the thing we need
        if base_file.is_file() && base_file != Path::new("") {
            // `.parent()` panics when:
            // - "/" is a directory, so this is not an issue
            // - "" is not an issue because of the `if` above
            // Thus this should never panic!
            base_file.parent().unwrap().join(path_offset)
        } else {
            base_file.join(path_offset)
        }
    }
}
