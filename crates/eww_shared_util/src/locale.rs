use chrono::Locale;
use std::env::var;

/// Returns the `Locale` enum based on the `LC_TIME` environment variable.
/// If the environment variable is not defined or is malformed use the POSIX locale.
pub fn get_locale() -> Locale {
    let locale_string: String =
        var("LC_TIME").map_or_else(|_| "C".to_string(), |v| v.split(".").next().unwrap_or("C").to_string());

    match (&*locale_string).try_into() {
        Ok(x) => x,
        Err(_) => Locale::POSIX,
    }
}
