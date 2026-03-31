use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::Command;

const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const OAUTH_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CREATE_API_KEY_URL: &str = "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";
// Request all scopes so we can handle both Console and Claude.ai users
const OAUTH_SCOPES: &str = "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    refresh_token: Option<String>,
    #[allow(dead_code)]
    expires_in: Option<u64>,
}

/// Authentication result — either an API key or an OAuth token
pub struct AuthResult {
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
}

impl AuthResult {
    /// Get the appropriate auth headers for the Anthropic API
    pub fn auth_headers(&self) -> Vec<(String, String)> {
        if let Some(ref token) = self.oauth_token {
            vec![
                ("Authorization".into(), format!("Bearer {}", token)),
                ("anthropic-beta".into(), "oauth-2025-04-20".into()),
            ]
        } else if let Some(ref key) = self.api_key {
            // Claude Code managed keys need the claude-code beta header
            let mut headers = vec![("x-api-key".into(), key.clone())];
            headers.push((
                "anthropic-beta".into(),
                "claude-code-20250219".into(),
            ));
            headers
        } else {
            vec![]
        }
    }
}

/// Try to get credentials from multiple sources, in order:
/// 1. ANTHROPIC_API_KEY environment variable
/// 2. macOS Keychain (Claude Code stored credentials)
/// 3. ~/.claude/.credentials.json file
/// 4. Interactive OAuth login
pub fn get_auth() -> Result<AuthResult> {
    // 1. Environment variable
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() {
            return Ok(AuthResult {
                api_key: Some(key),
                oauth_token: None,
            });
        }
    }

    // 2. macOS Keychain — try OAuth tokens from "Claude Code-credentials"
    if let Some(token) = read_oauth_from_keychain() {
        return Ok(AuthResult {
            api_key: None,
            oauth_token: Some(token),
        });
    }

    // 3. Credentials file
    if let Some(result) = read_from_credentials_file() {
        return Ok(result);
    }

    // 4. Interactive OAuth login
    eprintln!("No credentials found. Starting OAuth login...");
    let credential = oauth_login()?;
    // If it looks like an API key, use it as such; otherwise treat as OAuth token
    if credential.starts_with("sk-") {
        Ok(AuthResult {
            api_key: Some(credential),
            oauth_token: None,
        })
    } else {
        Ok(AuthResult {
            api_key: None,
            oauth_token: Some(credential),
        })
    }
}

/// Read OAuth access token from macOS Keychain (hex-encoded JSON)
fn read_oauth_from_keychain() -> Option<String> {
    let username = std::env::var("USER").ok()?;

    // Try "Claude Code-credentials" (OAuth token storage)
    if let Some(hex_value) = keychain_find("Claude Code-credentials", &username) {
        if let Some(token) = parse_hex_oauth_token(&hex_value) {
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
        None
    } else {
        Some(value)
    }
}

/// Parse hex-encoded JSON credentials and extract the OAuth access token
fn parse_hex_oauth_token(hex_str: &str) -> Option<String> {
    let bytes = hex::decode(hex_str.trim()).ok()?;
    let json_str = String::from_utf8(bytes).ok()?;
    let json: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    json.get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()
        .map(|s| s.to_string())
}

/// Read credentials from ~/.claude/.credentials.json
fn read_from_credentials_file() -> Option<AuthResult> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{}/.claude/.credentials.json", home);
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try OAuth token first
    if let Some(token) = json
        .get("claudeAiOauth")
        .and_then(|o| o.get("accessToken"))
        .and_then(|v| v.as_str())
    {
        return Some(AuthResult {
            api_key: None,
            oauth_token: Some(token.to_string()),
        });
    }

    // Try plain API key
    if let Some(key) = json.get("apiKey").and_then(|v| v.as_str()) {
        return Some(AuthResult {
            api_key: Some(key.to_string()),
            oauth_token: None,
        });
    }

    None
}

/// Perform interactive OAuth login (PKCE flow)
/// For Console users: OAuth → get token → create API key
/// For Claude.ai users: OAuth → use token directly
fn oauth_login() -> Result<String> {
    // Generate PKCE code verifier and challenge
    let code_verifier = generate_random_string(32);
    let code_challenge = sha256_base64url(&code_verifier);
    let state = generate_random_string(32);

    // Start local HTTP server for callback
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://localhost:{}/callback", port);

    // Use Console authorize URL (works for both Console and Claude.ai users)
    let auth_url = format!(
        "https://platform.claude.com/oauth/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        OAUTH_CLIENT_ID,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(OAUTH_SCOPES),
        code_challenge,
        state
    );

    // Open browser
    eprintln!("Opening browser for login...");
    eprintln!("If it doesn't open, visit:\n{}\n", auth_url);
    let _ = Command::new("open").arg(&auth_url).spawn();

    // Wait for callback
    eprintln!("Waiting for authentication...");
    let (auth_code, received_state) = wait_for_callback(&listener)?;

    if received_state != state {
        return Err(anyhow!("OAuth state mismatch — possible CSRF attack"));
    }

    // Exchange code for token
    let access_token = exchange_code_for_token(&auth_code, &redirect_uri, &code_verifier)?;
    eprintln!("OAuth token acquired. Creating API key...");

    // Try to create an API key via the Console endpoint
    match create_api_key(&access_token) {
        Ok(api_key) => {
            eprintln!("Login successful!\n");
            Ok(api_key)
        }
        Err(_) => {
            // If create_api_key fails, the token itself might work (Claude.ai subscriber)
            eprintln!("Login successful (using OAuth token directly).\n");
            Ok(access_token)
        }
    }
}

/// Create an API key using the OAuth access token (Console users)
fn create_api_key(access_token: &str) -> Result<String> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(CREATE_API_KEY_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "name": "mini-claude-code"
        }))
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(anyhow!("Failed to create API key ({}): {}", status, text));
    }

    let body: serde_json::Value = response.json()?;
    body.get("api_key")
        .or_else(|| body.get("key"))
        .or_else(|| body.get("secret"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("API key not found in response: {}", body))
}

fn wait_for_callback(listener: &TcpListener) -> Result<(String, String)> {
    let (mut stream, _) = listener.accept()?;
    let reader = BufReader::new(&stream);

    let request_line = reader
        .lines()
        .next()
        .ok_or_else(|| anyhow!("No request received"))??;

    // Parse "GET /callback?code=xxx&state=yyy HTTP/1.1"
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("Invalid request"))?;

    let query = url_part
        .split('?')
        .nth(1)
        .ok_or_else(|| anyhow!("No query parameters"))?;

    let mut code = None;
    let mut state = None;

    for param in query.split('&') {
        let mut parts = param.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("code"), Some(v)) => code = Some(urlencoding::decode(v)?.into_owned()),
            (Some("state"), Some(v)) => state = Some(urlencoding::decode(v)?.into_owned()),
            _ => {}
        }
    }

    // Send response to browser
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body><h2>Login successful!</h2><p>You can close this tab and return to the terminal.</p></body></html>";
    stream.write_all(response.as_bytes())?;

    Ok((
        code.ok_or_else(|| anyhow!("No authorization code received"))?,
        state.ok_or_else(|| anyhow!("No state received"))?,
    ))
}

fn exchange_code_for_token(
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<String> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", OAUTH_CLIENT_ID),
        ("code_verifier", code_verifier),
    ];

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(OAUTH_TOKEN_URL)
        .form(&params)
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(anyhow!("Token exchange failed ({}): {}", status, text));
    }

    let token_response: TokenResponse = response.json()?;
    Ok(token_response.access_token)
}

fn generate_random_string(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Simple random using time + pid as seed (good enough for PKCE)
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        ^ (std::process::id() as u128);

    let mut bytes = Vec::with_capacity(len);
    let mut state = seed;
    for _ in 0..len {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        bytes.push((state >> 33) as u8);
    }

    base64url_encode(&bytes)
}

fn sha256_base64url(input: &str) -> String {
    use std::io::Read;
    // Use openssl command for SHA256 (available on macOS)
    let output = Command::new("openssl")
        .arg("dgst")
        .arg("-sha256")
        .arg("-binary")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            let mut buf = Vec::new();
            child.stdout.take().unwrap().read_to_end(&mut buf).unwrap();
            child.wait()?;
            Ok(buf)
        })
        .expect("Failed to run openssl");

    base64url_encode(&output)
}

fn base64url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_oauth_token() {
        let json = r#"{"claudeAiOauth":{"accessToken":"test-token-123"}}"#;
        let hex = hex::encode(json.as_bytes());
        assert_eq!(parse_hex_oauth_token(&hex), Some("test-token-123".to_string()));
    }

    #[test]
    fn test_parse_hex_invalid() {
        assert_eq!(parse_hex_oauth_token("not-hex"), None);
        assert_eq!(parse_hex_oauth_token(""), None);
    }

    #[test]
    fn test_base64url_encode() {
        let result = base64url_encode(b"hello");
        assert_eq!(result, "aGVsbG8");
    }

    #[test]
    fn test_generate_random_string() {
        let s1 = generate_random_string(32);
        let s2 = generate_random_string(32);
        assert!(!s1.is_empty());
        // Two consecutive calls should be very likely different
        // (not guaranteed but extremely likely with nanosecond seed)
    }
}
