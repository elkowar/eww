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
            log::error!("[{}:{}] Error while {}: {:?}", ::std::file!(), ::std::line!(), $context, err);
        }
    }};
}

#[macro_export]
macro_rules! print_result_err {
    ($context:expr, $result:expr $(,)?) => {{
        if let Err(err) = $result {
            log::error!("[{}:{}] Error {}: {:?}", ::std::file!(), ::std::line!(), $context, err);
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

/// Parse a string with a concrete set of options into some data-structure,
/// and return a nicely formatted error message on invalid values. I.e.:
/// ```rs
/// let input = "up";
/// enum_parse { "direction", input,
///   "up" => Direction::Up,
///   "down" => Direction::Down,
/// }
/// ```
#[macro_export]
macro_rules! enum_parse {
    ($name:literal, $input:expr, $($($s:literal)|* => $val:expr),* $(,)?) => {
        let input = $input;
        match input {
            $( $( $s )|* => Ok($val) ),*,
            _ => Err(anyhow!(concat!("Couldn't parse ", $name, ": '{}'. Possible values are ", $($($s),*),*), input))
        }
    };
}

/// Compute the difference of two lists, returning a tuple of
/// (
///   elements that where in a but not in b,
///   elements that where in b but not in a
/// ).
pub fn list_difference<'a, 'b, T: PartialEq>(a: &'a [T], b: &'b [T]) -> (Vec<&'a T>, Vec<&'b T>) {
    let mut missing = Vec::new();
    for elem in a {
        if !b.contains(elem) {
            missing.push(elem);
        }
    }

    let mut new = Vec::new();
    for elem in b {
        if !a.contains(elem) {
            new.push(elem);
        }
    }
    (missing, new)
}

/// Joins two paths while keeping it somewhat pretty.
/// If the second path is absolute, this will just return the second path.
/// If it is relative, it will return the second path joined onto the first path, removing any `./` if present.
/// TODO this is not yet perfect, as it will still leave ../ and multiple ./ etc,... check for a Path::simplify or something.
pub fn join_path_pretty<P: AsRef<std::path::Path>, P2: AsRef<std::path::Path>>(a: P, b: P2) -> std::path::PathBuf {
    let a = a.as_ref();
    let b = b.as_ref();
    if b.is_absolute() {
        b.to_path_buf()
    } else {
        a.parent().unwrap().join(b.strip_prefix("./").unwrap_or(b))
    }
}

/// extends a hashmap, returning a list of keys that already where present in the hashmap.
pub fn extend_safe<K: std::cmp::Eq + std::hash::Hash + Clone, V, T: IntoIterator<Item = (K, V)>>(
    a: &mut std::collections::HashMap<K, V>,
    b: T,
) -> Vec<K> {
    b.into_iter().filter_map(|(k, v)| a.insert(k.clone(), v).map(|_| k.clone())).collect()
}

/// read an scss file, replace all environment variable references within it and
/// then parse it into css.
pub fn parse_scss_from_file(path: &Path) -> Result<String> {
    let config_dir = path.parent().context("Given SCSS file has no parent directory?!")?;
    let scss_file_content =
        std::fs::read_to_string(path).with_context(|| format!("Given SCSS File Doesnt Exist! {}", path.display()))?;
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
    } else if s.ends_with('s') {
        Ok(Duration::from_secs(s.trim_end_matches('s').parse()?))
    } else if s.ends_with('m') {
        Ok(Duration::from_secs(s.trim_end_matches('m').parse::<u64>()? * 60))
    } else if s.ends_with('h') {
        Ok(Duration::from_secs(s.trim_end_matches('h').parse::<u64>()? * 60 * 60))
    } else {
        Err(anyhow!("unrecognized time format: {}", s))
    }
}

pub trait IterAverage {
    fn avg(self) -> f32;
}

impl<I: Iterator<Item = f32>> IterAverage for I {
    fn avg(self) -> f32 {
        let mut total = 0f32;
        let mut cnt = 0f32;
        for value in self {
            total += value;
            cnt += 1f32;
        }
        total / cnt
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
        .replace_all(&input, |var_name: &regex::Captures| std::env::var(var_name.get(1).unwrap().as_str()).unwrap_or_default())
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
