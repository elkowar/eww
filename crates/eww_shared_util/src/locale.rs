use chrono::Locale;
use std::env::var;

/// Returns the `Locale` enum based on the `LC_ALL`, `LC_TIME`, and `LANG` environment variables in
/// that order, which is the precedence order prescribed by Section 8.2 of POSIX.1-2008.
/// If the environment variable is not defined or is malformed use the POSIX locale.
pub fn get_locale() -> Locale {
    let locale_env = var("LC_ALL").or_else(|_| var("LC_TIME").or_else(|_| var("LANG")));
    let locale_string: String = locale_env.map_or_else(|_| "C".to_string(), |v| v.split(".").next().unwrap_or("C").to_string());

    match (&*locale_string).try_into() {
        Ok(x) => x,
        Err(_) => Locale::POSIX,
    }
}
