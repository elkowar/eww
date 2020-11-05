use anyhow::*;
use extend::ext;
use grass;
use itertools::Itertools;
use std::path::Path;

#[macro_export]
macro_rules! impl_try_from {
    ($typ:ty {
        $(
            for $for:ty => |$arg:ident| $code:expr
        );*;
    }) => {
        $(impl TryFrom<$typ> for $for {
            type Error = anyhow::Error;

            fn try_from($arg: $typ) -> Result<Self> {
                $code
            }
        })*
    };
}

#[macro_export]
macro_rules! try_logging_errors {
    ($context:literal => $code:block) => {{
        let result: Result<_> = try { $code };
        if let Err(err) = result {
            eprintln!("Error while {}: {:?}", $context, err);
        }
    }};
}

/// read an scss file, replace all environment variable references within it and
/// then parse it into css.
pub fn parse_scss_from_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let scss_content = replace_env_var_references(std::fs::read_to_string(path)?);
    grass::from_string(scss_content, &grass::Options::default()).map_err(|err| anyhow!("encountered SCSS parsing error: {}", err))
}

#[ext(pub, name = StringExt)]
impl<T: AsRef<str>> T {
    /// check if the string is empty after removing all linebreaks and trimming
    /// whitespace
    fn is_blank(self) -> bool {
        self.as_ref().replace('\n', "").trim().is_empty()
    }

    /// trim all lines in a string
    fn trim_lines(self) -> String {
        self.as_ref().lines().map(|line| line.trim()).join("\n")
    }
}

pub fn parse_duration(s: &str) -> Result<std::time::Duration> {
    use std::time::Duration;
    if s.ends_with("ms") {
        Ok(Duration::from_millis(s.trim_end_matches("ms").parse()?))
    } else if s.ends_with("s") {
        Ok(Duration::from_secs(s.trim_end_matches("s").parse()?))
    } else if s.ends_with("m") {
        Ok(Duration::from_secs(s.trim_end_matches("m").parse::<u64>()? * 60))
    } else if s.ends_with("h") {
        Ok(Duration::from_secs(s.trim_end_matches("h").parse::<u64>()? * 60 * 60))
    } else {
        Err(anyhow!("unrecognized time format: {}", s))
    }
}

/// Replace all env-var references of the format `"something $foo"` in a string
/// by the actual env-variables. If the env-var isn't found, will replace the
/// reference with an empty string.
pub fn replace_env_var_references(input: String) -> String {
    lazy_static::lazy_static! {
        static ref ENV_VAR_PATTERN: regex::Regex = regex::Regex::new(r"\$([^\s]*)").unwrap();
    }
    ENV_VAR_PATTERN
        .replace_all(&input, |var_name: &regex::Captures| {
            std::env::var(var_name.get(1).unwrap().as_str()).unwrap_or_default()
        })
        .into_owned()
}

/// If the given result is `Err`, prints out the error value using `{:?}`
pub fn print_result_err<T, E: std::fmt::Debug>(context: &str, result: &std::result::Result<T, E>) {
    if let Err(err) = result {
        eprintln!("Error {}: {:?}", context, err);
    }
}

pub fn config_path() -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let config_file_path = crate::CONFIG_DIR.join("eww.xml");
    let config_dir = config_file_path
        .parent()
        .context("config file did not have a parent?!")?
        .to_owned()
        .to_path_buf();
    let scss_file_path = config_dir.join("eww.scss");
    Ok((config_file_path, scss_file_path))
}

pub fn input() -> Result<String> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input)
}
pub fn launch_editor(editor: &String, path: &str) -> Result<std::process::ExitStatus> {
    // have to do this so that the EDITOR environment variable,
    // gets parsed as one and not as several args,
    // so that e.g. your EDITOR env variable is equal to `vim -xx`
    // then that gets started as such and not as the binary `vim -xx`
    Ok(std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} {}", editor, path))
        .spawn()?
        .wait()?)
}
