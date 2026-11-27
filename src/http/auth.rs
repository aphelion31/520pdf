//! Argon2id password hashing, HMAC-SHA256 session authentication, and API token lookup.
use crate::http::tokens::{TokenAuthError, TokenIdentity, TokenStore};
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
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;
const SESSION_COOKIE: &str = "520pdf_session";
const SESSION_TTL_SECS: u64 = 60 * 60 * 24 * 7;

#[derive(Clone)]
pub struct AuthConfig {
    enabled: bool,
    registration_enabled: bool,
    username: String,
    password_hash: String,
    signing_key: Vec<u8>,
    store: Arc<TokenStore>,
}

#[cfg(test)]
fn temp_test_data_dir() -> PathBuf {
    let n: u64 = rand::random();
    std::env::temp_dir().join(format!("520pdf-auth-{}-{n}", std::process::id()))
}

impl AuthConfig {
    #[cfg(test)]
    pub fn disabled() -> Self {
        Self::disabled_with_dir(temp_test_data_dir())
    }

    #[cfg(test)]
    pub fn disabled_with_dir(data_dir: PathBuf) -> Self {
        let store = TokenStore::load_or_init(data_dir).expect("init token store");
        let signing_key = store.signing_secret();
        Self {
            enabled: false,
            registration_enabled: true,
            username: String::new(),
            password_hash: String::new(),
            signing_key,
            store: Arc::new(store),
        }
    }

    pub fn from_env() -> Result<Self, String> {
        let data_dir = PathBuf::from(
            std::env::var("DATA_DIR")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| ".520pdf-data".to_string()),
        );
        let mut store = TokenStore::load_or_init(data_dir)?;
        store.seed_cors_origins_from_env()?;
        let require_api_key = matches!(
            std::env::var("REQUIRE_API_KEY").ok().as_deref(),
            Some("true" | "1")
        );
        let user = std::env::var("AUTH_USER")
            .ok()
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        let env_admin_enabled = !user.is_empty();
        let password_hash = if env_admin_enabled {
            match std::env::var("AUTH_PASSWORD_HASH")
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
            }
        } else {
            String::new()
        };
        let signing_key = match std::env::var("AUTH_SECRET")
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            Some(s) => {
                store.set_seal_pepper(s.as_bytes());
                s.into_bytes()
            }
            None => store.signing_secret(),
        };
        let disable_reg = matches!(
            std::env::var("DISABLE_REGISTRATION").ok().as_deref(),
            Some("true" | "1")
        );
        Ok(Self {
            enabled: env_admin_enabled || require_api_key,
            registration_enabled: !env_admin_enabled && !disable_reg,
            username: user,
            password_hash,
            signing_key,
            store: Arc::new(store),
        })
    }

    pub fn store(&self) -> &Arc<TokenStore> {
        &self.store
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_authorized(&self, headers: &HeaderMap) -> bool {
        !self.enabled || self.current_user(headers).is_some()
    }

    pub fn verify_api_token(&self, raw: &str, tool: &str) -> Result<TokenIdentity, TokenAuthError> {
        self.store.authenticate_token(raw, tool)
    }

    pub fn current_user(&self, headers: &HeaderMap) -> Option<String> {
        let token = extract_cookie(headers, SESSION_COOKIE)?;
        self.verify_session_token(&token)
    }

    pub fn verify_user_password(&self, username: &str, password: &str) -> bool {
        if password.is_empty() {
            return false;
        }
        if !self.username.is_empty()
            && constant_time_eq(username.as_bytes(), self.username.as_bytes())
        {
            return verify_password(password, &self.password_hash);
        }
        self.store.verify_stored_user(username, password).is_some()
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
        if (!self.username.is_empty()
            && constant_time_eq(user.as_bytes(), self.username.as_bytes()))
            || self.store.has_user(&user)
        {
            Some(user)
        } else {
            None
        }
    }
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Extract API key from `Authorization: Bearer <token>` or `X-API-Key: <token>`.
pub fn extract_api_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        && let Some(bearer) = auth
            .trim()
            .strip_prefix("Bearer ")
            .or_else(|| auth.trim().strip_prefix("bearer "))
    {
        return Some(bearer.trim().to_string());
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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
        "registration_enabled": auth.registration_enabled,
        "api_keys_require_login": true,
        "has_users": auth.store.has_any_users() || !auth.username.is_empty(),
    }))
}

fn issue_session_response(auth: &AuthConfig, username: &str, status: StatusCode) -> Response {
    let exp = now_unix() + SESSION_TTL_SECS;
    let token = auth.sign_session_token(username, exp);
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_TTL_SECS}"
    );
    let mut res = (
        status,
        Json(json!({
            "authenticated": true,
            "username": username,
            "algorithm": "argon2id",
        })),
    )
        .into_response();
    if let Ok(val) = HeaderValue::from_str(&cookie) {
        res.headers_mut().insert(header::SET_COOKIE, val);
    }
    res
}

pub async fn register(
    State(auth): State<Arc<AuthConfig>>,
    payload: Result<Json<LoginRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if !auth.registration_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Self-registration is disabled on this server"})),
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
    let reserved = (!auth.username.is_empty()).then_some(auth.username.as_str());
    match auth
        .store
        .register_user(&req.username, &req.password, reserved)
    {
        Ok(user) => issue_session_response(&auth, &user.username, StatusCode::CREATED),
        Err(msg) => {
            let status = if msg.contains("exists") || msg.contains("reserved") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(json!({"error": msg}))).into_response()
        }
    }
}

pub async fn login(
    State(auth): State<Arc<AuthConfig>>,
    payload: Result<Json<LoginRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(req)) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Provide JSON { username, password }"})),
        )
            .into_response();
    };
    if !auth.username.is_empty()
        && constant_time_eq(req.username.as_bytes(), auth.username.as_bytes())
        && verify_password(&req.password, &auth.password_hash)
    {
        return issue_session_response(&auth, &auth.username, StatusCode::OK);
    }
    if let Some(canonical) = auth.store.verify_stored_user(&req.username, &req.password) {
        return issue_session_response(&auth, &canonical, StatusCode::OK);
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "Invalid credentials"})),
    )
        .into_response()
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
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::load_or_init(dir.path().to_path_buf()).unwrap();
        let hash = hash_password("Secret-520!").unwrap();
        let cfg = AuthConfig {
            enabled: true,
            registration_enabled: false,
            username: "admin".into(),
            password_hash: hash,
            signing_key: b"test-signing-key-32-bytes-long!!".to_vec(),
            store: Arc::new(store),
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
