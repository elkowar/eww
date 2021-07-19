pub fn parse_duration(s: &str) -> anyhow::Result<std::time::Duration> {
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
        Err(anyhow::anyhow!("Failed to parse duration `{}`", s))
    }
}
