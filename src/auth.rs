use axum::http::HeaderMap;

use crate::config::AuthConfig;

pub fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

pub fn is_authorized(headers: &HeaderMap, auth: &AuthConfig) -> bool {
    if auth.virtual_keys.is_empty() {
        return true;
    }
    match extract_bearer(headers) {
        Some(key) => auth.virtual_keys.iter().any(|k| k == &key),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::*;

    #[test]
    fn validates_virtual_key() {
        let mut headers = HeaderMap::new();
        let bearer = format!("{} {}", "Bearer", "rk_test");
        headers.insert(
            "authorization",
            HeaderValue::from_str(&bearer).expect("valid auth header"),
        );
        let auth = AuthConfig {
            virtual_keys: vec!["rk_test".to_string()],
        };
        assert!(is_authorized(&headers, &auth));
    }
}
