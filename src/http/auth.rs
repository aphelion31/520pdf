//! Argon2id password hashing and HMAC-SHA256 session authentication.
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;
const SESSION_COOKIE: &str = "520pdf_session";
const SESSION_TTL_SECS: u64 = 60 * 60 * 24 * 7;

#[derive(Clone)]
pub struct AuthConfig {
    enabled: bool,
    username: String,
    password_hash: String,
    signing_key: Vec<u8>,
}

impl AuthConfig {
    pub fn disabled() -> Self {
        let mut key = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        Self {
            enabled: false,
            username: String::new(),
            password_hash: String::new(),
            signing_key: key,
        }
    }

    pub fn from_env() -> Result<Self, String> {
        let Ok(user) = std::env::var("AUTH_USER").map(|v| v.trim().to_string()) else {
            return Ok(Self::disabled());
        };
        if user.is_empty() {
            return Ok(Self::disabled());
        }
        let password_hash = match std::env::var("AUTH_PASSWORD_HASH")
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            Some(hash) => {
                validate_argon2id_phc(&hash)?;
                hash
            }
            None => {
                let raw = std::env::var("AUTH_PASSWORD").ok().unwrap_or_default();
                if raw.is_empty() {
                    return Err(
                        "AUTH_USER is set without AUTH_PASSWORD_HASH or AUTH_PASSWORD".into(),
                    );
                }
                hash_password(&raw)?
            }
        };
        let signing_key = match std::env::var("AUTH_SECRET")
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            Some(s) => s.into_bytes(),
            None => {
                let mut key = vec![0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                key
            }
        };
        Ok(Self {
            enabled: true,
            username: user,
            password_hash,
            signing_key,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_authorized(&self, headers: &HeaderMap) -> bool {
        !self.enabled || self.current_user(headers).is_some()
    }

    pub fn current_user(&self, headers: &HeaderMap) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let token = extract_cookie(headers, SESSION_COOKIE)?;
        self.verify_session_token(&token)
    }

    fn sign_session_token(&self, username: &str, exp_unix: u64) -> String {
        let user_hex = hex::encode(username.as_bytes());
        let payload = format!("{user_hex}.{exp_unix}");
        let mut mac =
            HmacSha256::new_from_slice(&self.signing_key).expect("HMAC takes any key length");
        mac.update(payload.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        format!("{payload}.{sig}")
    }

    fn verify_session_token(&self, token: &str) -> Option<String> {
        let mut parts = token.split('.');
        let user_hex = parts.next()?;
        let exp_str = parts.next()?;
        let sig_hex = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let exp = exp_str.parse::<u64>().ok()?;
        if now_unix() >= exp {
            return None;
        }
        let payload = format!("{user_hex}.{exp_str}");
        let mut mac =
            HmacSha256::new_from_slice(&self.signing_key).expect("HMAC takes any key length");
        mac.update(payload.as_bytes());
        let expected_sig = mac.finalize().into_bytes();
        let actual_sig = hex::decode(sig_hex).ok()?;
        if !constant_time_eq(&expected_sig, &actual_sig) {
            return None;
        }
        let user_bytes = hex::decode(user_hex).ok()?;
        let user = String::from_utf8(user_bytes).ok()?;
        if constant_time_eq(user.as_bytes(), self.username.as_bytes()) {
            Some(user)
        } else {
            None
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Constant-time comparison helper for secrets, usernames, and HMAC digests.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn validate_argon2id_phc(phc: &str) -> Result<(), String> {
    let parsed = PasswordHash::new(phc).map_err(|_| "Invalid Argon2id PHC password hash")?;
    if parsed.algorithm.as_str() != "argon2id" {
        return Err("Password hash must use algorithm argon2id".into());
    }
    Ok(())
}

/// Hash a password with Argon2id (v0x13, default OWASP parameters) into a PHC string.
pub fn hash_password(password: &str) -> Result<String, String> {
    if password.is_empty() {
        return Err("Password cannot be empty".into());
    }
    let mut salt_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| e.to_string())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verify a candidate password strictly against an `$argon2id$...` PHC hash.
pub fn verify_password(password: &str, phc_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc_hash) else {
        return false;
    };
    if parsed.algorithm.as_str() != "argon2id" {
        return false;
    }
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in raw.split(';') {
        let Some((k, v)) = pair.trim().split_once('=') else {
            continue;
        };
        if k.trim() == name {
            return Some(v.trim().to_string());
        }
    }
    None
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

pub async fn me(
    State(auth): State<Arc<AuthConfig>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let user = auth.current_user(&headers);
    Json(json!({
        "auth_enabled": auth.is_enabled(),
        "authenticated": user.is_some(),
        "username": user,
        "algorithm": "argon2id",
    }))
}

pub async fn login(
    State(auth): State<Arc<AuthConfig>>,
    payload: Result<Json<LoginRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if !auth.is_enabled() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Authentication is not enabled on this server"})),
        )
            .into_response();
    }
    let Ok(Json(req)) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Provide JSON { username, password }"})),
        )
            .into_response();
    };
    let user_ok = constant_time_eq(req.username.as_bytes(), auth.username.as_bytes());
    let pass_ok = verify_password(&req.password, &auth.password_hash);
    if !user_ok || !pass_ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"})),
        )
            .into_response();
    }
    let exp = now_unix() + SESSION_TTL_SECS;
    let token = auth.sign_session_token(&auth.username, exp);
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_TTL_SECS}"
    );
    let mut res = (
        StatusCode::OK,
        Json(json!({
            "authenticated": true,
            "username": auth.username,
            "algorithm": "argon2id",
        })),
    )
        .into_response();
    if let Ok(val) = HeaderValue::from_str(&cookie) {
        res.headers_mut().insert(header::SET_COOKIE, val);
    }
    res
}

pub async fn logout() -> Response {
    let mut res = (StatusCode::OK, Json(json!({ "authenticated": false }))).into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("520pdf_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
    );
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2id_hash_and_verify_roundtrip() {
        let hash = hash_password("CorrectHorse520!").unwrap();
        assert!(hash.starts_with("$argon2id$v=19$"));
        assert!(verify_password("CorrectHorse520!", &hash));
        assert!(!verify_password("WrongPassword", &hash));
        assert!(!verify_password("CorrectHorse520!", "plaintext"));
        assert!(!verify_password(
            "CorrectHorse520!",
            "$2b$12$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ01234"
        ));
        assert!(hash_password("").is_err());
    }

    #[test]
    fn session_cookie_sign_verify_and_tamper_rejection() {
        let hash = hash_password("Secret-520!").unwrap();
        let cfg = AuthConfig {
            enabled: true,
            username: "admin".into(),
            password_hash: hash,
            signing_key: b"test-signing-key-32-bytes-long!!".to_vec(),
        };
        let token = cfg.sign_session_token("admin", now_unix() + 600);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
        );
        assert_eq!(cfg.current_user(&headers).as_deref(), Some("admin"));
        assert!(cfg.is_authorized(&headers));

        let tampered = format!("{token}00");
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={tampered}")).unwrap(),
        );
        assert_eq!(cfg.current_user(&headers), None);
        assert!(!cfg.is_authorized(&headers));

        let expired = cfg.sign_session_token("admin", 1);
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={expired}")).unwrap(),
        );
        assert!(!cfg.is_authorized(&headers));
    }
}
