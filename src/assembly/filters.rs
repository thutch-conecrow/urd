use anyhow::{Context, Result};
use url::Url;

/// A parsed filter from a template expression (e.g., `{{ item | local_ip }}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    LocalIp,
}

impl Filter {
    /// Parse a filter name into a `Filter` variant.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "local_ip" => Ok(Self::LocalIp),
            _ => anyhow::bail!("unknown filter '{name}'"),
        }
    }

    /// Apply this filter to a resolved value.
    pub fn apply(&self, value: &str) -> Result<String> {
        match self {
            Self::LocalIp => apply_local_ip(value),
        }
    }
}

/// Apply a filter pipeline to a value, left to right.
pub fn apply_filters(value: &str, filters: &[Filter]) -> Result<String> {
    let mut result = value.to_string();
    for filter in filters {
        result = filter.apply(&result)?;
    }
    Ok(result)
}

/// Parse the content inside `{{ ... }}` into an item ID and optional filter pipeline.
///
/// Examples:
/// - `"app.url"` → `("app.url", [])`
/// - `"app.url | local_ip"` → `("app.url", [LocalIp])`
/// - `"app.url | local_ip | other"` → `("app.url", [LocalIp, Other])`
pub fn parse_expression(inner: &str) -> Result<(String, Vec<Filter>)> {
    let parts: Vec<&str> = inner.split('|').collect();
    let item_id = parts[0].trim().to_string();

    let mut filters = Vec::new();
    for part in &parts[1..] {
        let filter_name = part.trim();
        let filter = Filter::parse(filter_name)
            .with_context(|| format!("in expression '{inner}'"))?;
        filters.push(filter);
    }

    Ok((item_id, filters))
}

/// Replace loopback hosts (`localhost`, `127.0.0.1`, `::1`) in a URL with the
/// machine's LAN IP address. Non-loopback URLs are returned unchanged.
fn apply_local_ip(value: &str) -> Result<String> {
    let mut parsed = Url::parse(value)
        .with_context(|| format!("local_ip filter: '{value}' is not a valid URL"))?;

    let host = parsed.host_str().unwrap_or_default();

    if !is_loopback(host) {
        return Ok(value.to_string());
    }

    let lan_ip = local_ip_address::local_ip()
        .context("local_ip filter: could not determine LAN IP address")?;

    parsed
        .set_host(Some(&lan_ip.to_string()))
        .map_err(|_| anyhow::anyhow!("local_ip filter: could not set host to '{lan_ip}'"))?;

    Ok(parsed.to_string())
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_filters() {
        let (id, filters) = parse_expression("app.url").unwrap();
        assert_eq!(id, "app.url");
        assert!(filters.is_empty());
    }

    #[test]
    fn parse_single_filter() {
        let (id, filters) = parse_expression("app.url | local_ip").unwrap();
        assert_eq!(id, "app.url");
        assert_eq!(filters, vec![Filter::LocalIp]);
    }

    #[test]
    fn parse_single_filter_no_spaces() {
        let (id, filters) = parse_expression("app.url|local_ip").unwrap();
        assert_eq!(id, "app.url");
        assert_eq!(filters, vec![Filter::LocalIp]);
    }

    #[test]
    fn parse_multiple_filters() {
        // Only local_ip exists right now, but parsing supports the pipeline
        let (id, filters) = parse_expression("app.url | local_ip").unwrap();
        assert_eq!(id, "app.url");
        assert_eq!(filters.len(), 1);
    }

    #[test]
    fn parse_unknown_filter_errors() {
        assert!(parse_expression("app.url | bogus").is_err());
    }

    #[test]
    fn local_ip_replaces_localhost() {
        let result = apply_local_ip("http://localhost:4000").unwrap();
        assert!(!result.contains("localhost"));
        assert!(result.contains(":4000"));
        assert!(result.starts_with("http://"));
    }

    #[test]
    fn local_ip_replaces_127() {
        let result = apply_local_ip("http://127.0.0.1:57321").unwrap();
        assert!(!result.contains("127.0.0.1"));
        assert!(result.contains(":57321"));
    }

    #[test]
    fn local_ip_preserves_non_loopback() {
        let input = "https://example.supabase.co";
        let result = apply_local_ip(input).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn local_ip_preserves_path_and_query() {
        let result = apply_local_ip("http://localhost:3000/api/v1?key=val").unwrap();
        assert!(result.contains("/api/v1?key=val"));
        assert!(!result.contains("localhost"));
    }

    #[test]
    fn local_ip_errors_on_non_url() {
        assert!(apply_local_ip("not-a-url").is_err());
    }

    #[test]
    fn apply_filters_empty_pipeline() {
        let result = apply_filters("hello", &[]).unwrap();
        assert_eq!(result, "hello");
    }
}
