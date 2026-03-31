use std::process::Command;

/// Try to get the API key from multiple sources, in order:
/// 1. ANTHROPIC_API_KEY environment variable
/// 2. macOS Keychain ("Claude Code" service)
/// 3. ~/.claude/.credentials file
pub fn get_api_key() -> Result<String, String> {
    // 1. Environment variable
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // 2. macOS Keychain
    if let Some(key) = read_from_keychain() {
        return Ok(key);
    }

    // 3. Credentials file
    if let Some(key) = read_from_credentials_file() {
        return Ok(key);
    }

    Err(
        "No API key found. Either:\n  \
         - Set ANTHROPIC_API_KEY environment variable\n  \
         - Login via Claude Code (`claude` CLI → /login)\n  \
         - Place your key in ~/.claude/.credentials.json"
            .to_string(),
    )
}

/// Read API key or OAuth token from macOS Keychain.
/// Claude Code stores credentials under the "Claude Code" service.
fn read_from_keychain() -> Option<String> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()?;

    // Try "Claude Code" service first (stores API key directly)
    if let Some(value) = keychain_find("Claude Code", &username) {
        // Could be a plain API key or hex-encoded JSON
        if value.starts_with("sk-") {
            return Some(value);
        }
        // Try hex-decode as JSON (OAuth token storage format)
        if let Some(token) = parse_hex_credentials(&value) {
            return Some(token);
        }
    }

    // Try "Claude Code-credentials" service (hex-encoded JSON with OAuth tokens)
    if let Some(value) = keychain_find("Claude Code-credentials", &username) {
        if let Some(token) = parse_hex_credentials(&value) {
            return Some(token);
        }
    }

    None
}

fn keychain_find(service: &str, account: &str) -> Option<String> {
    let output = Command::new("security")
        .arg("find-generic-password")
        .arg("-s")
        .arg(service)
        .arg("-a")
        .arg(account)
        .arg("-w")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        return None;
    }

    Some(value)
}

/// Parse hex-encoded JSON credentials and extract the OAuth access token.
/// Claude Code stores: hex(JSON({ claudeAiOauth: { accessToken, ... } }))
fn parse_hex_credentials(hex_str: &str) -> Option<String> {
    let bytes = hex::decode(hex_str.trim()).ok()?;
    let json_str = String::from_utf8(bytes).ok()?;
    let json: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    json.get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()
        .map(|s| s.to_string())
}

/// Read credentials from ~/.claude/.credentials.json (plaintext fallback)
fn read_from_credentials_file() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{}/.claude/.credentials.json", home);
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try OAuth token
    if let Some(token) = json
        .get("claudeAiOauth")
        .and_then(|o| o.get("accessToken"))
        .and_then(|v| v.as_str())
    {
        return Some(token.to_string());
    }

    // Try plain API key
    json.get("apiKey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_credentials() {
        // JSON: {"claudeAiOauth":{"accessToken":"test-token-123"}}
        let json = r#"{"claudeAiOauth":{"accessToken":"test-token-123"}}"#;
        let hex = hex::encode(json.as_bytes());
        let result = parse_hex_credentials(&hex);
        assert_eq!(result, Some("test-token-123".to_string()));
    }

    #[test]
    fn test_parse_hex_credentials_invalid() {
        assert_eq!(parse_hex_credentials("not-hex"), None);
        assert_eq!(parse_hex_credentials(""), None);
    }
}
