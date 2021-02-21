use anyhow::*;
use extend::ext;
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
    ($context:expr => $code:block) => {{
        let result: Result<_> = try { $code };
        if let Err(err) = result {
            eprintln!("[{}:{}] Error while {}: {:?}", ::std::file!(), ::std::line!(), $context, err);
        }
    }};
}

#[macro_export]
macro_rules! print_result_err {
    ($context:expr, $result:expr $(,)?) => {{
        if let Err(err) = $result {
            eprintln!("[{}:{}] Error {}: {:?}", ::std::file!(), ::std::line!(), $context, err);
        }
    }};
}

#[macro_export]
macro_rules! loop_select {
    ($($body:tt)*) => {
        loop {
            ::tokio::select! {
                $($body)*
            };
        }
    }
}

/// read an scss file, replace all environment variable references within it and
/// then parse it into css.
pub fn parse_scss_from_file(path: &Path) -> Result<String> {
    let config_dir = path.parent().context("Given SCSS file has no parent directory?!")?;
    let scss_file_content = std::fs::read_to_string(path).with_context(|| { format!("Given SCSS File Doesnt Exist! {}", path.display()) })?;
    let file_content = replace_env_var_references(scss_file_content);
    let grass_config = grass::Options::default().load_path(config_dir);
    grass::from_string(file_content, &grass_config).map_err(|err| anyhow!("Encountered SCSS parsing error: {:?}", err))
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

/// Replace all env-var references of the format `"something ${foo}"` in a string
/// by the actual env-variables. If the env-var isn't found, will replace the
/// reference with an empty string.
pub fn replace_env_var_references(input: String) -> String {
    lazy_static::lazy_static! {
        static ref ENV_VAR_PATTERN: regex::Regex = regex::Regex::new(r"\$\{([^\s]*)\}").unwrap();
    }
    ENV_VAR_PATTERN
        .replace_all(&input, |var_name: &regex::Captures| {
            std::env::var(var_name.get(1).unwrap().as_str()).unwrap_or_default()
        })
        .into_owned()
}

#[cfg(test)]
mod test {
    use super::replace_env_var_references;
    use std;

    #[test]
    fn test_replace_env_var_references() {
        let scss = "$test: ${USER};";

        assert_eq!(
            replace_env_var_references(String::from(scss)),
            format!("$test: {};", std::env::var("USER").unwrap_or_default())
        )
    }
}
