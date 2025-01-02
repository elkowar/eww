use chrono::Locale;
use std::env::var;

/// Returns the `Locale` enum based on the `LC_ALL`, `LC_TIME`, and `LANG` environment variables in
/// that order, which is the precedence order prescribed by Section 8.2 of POSIX.1-2017.
/// If the environment variable is not defined or is malformed use the POSIX locale.
pub fn get_locale() -> Locale {
    var("LC_ALL")
        .or_else(|_| var("LC_TIME"))
        .or_else(|_| var("LANG"))
        .map_or(Locale::POSIX, |v| v.split('.').next().and_then(|x| x.try_into().ok()).unwrap_or_default())
}
