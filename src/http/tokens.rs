//! Persistent Argon2id user accounts and SHA-256 hashed scoped API tokens.
use crate::http::auth::{AuthConfig, constant_time_eq, hash_password, now_unix, verify_password};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

type HmacSha256 = Hmac<Sha256>;
const MAX_TOKENS_PER_USER: usize = 50;

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredUser {
    pub username: String,
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub prefix: String,
    pub token_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_token: Option<String>,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub expires_at: Option<u64>,
    pub last_used_at: Option<String>,
    pub usage_count: u64,
    pub revoked: bool,
}

#[derive(Clone, Serialize)]
pub struct TokenView {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub expires_at: Option<u64>,
    pub last_used_at: Option<String>,
    pub usage_count: u64,
    pub revoked: bool,
    pub revealable: bool,
}

impl From<&StoredToken> for TokenView {
    fn from(t: &StoredToken) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            prefix: t.prefix.clone(),
            scopes: t.scopes.clone(),
            created_at: t.created_at.clone(),
            expires_at: t.expires_at,
            last_used_at: t.last_used_at.clone(),
            usage_count: t.usage_count,
            revoked: t.revoked,
            revealable: !t.revoked && t.encrypted_token.is_some(),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct StoreData {
    #[serde(default)]
    signing_secret_hex: String,
    #[serde(default)]
    users: Vec<StoredUser>,
    #[serde(default)]
    tokens: Vec<StoredToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cors_origins: Option<Vec<String>>,
}

const MAX_CORS_ORIGINS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorsMatch {
    Explicit,
    Wildcard,
    Denied,
}

/// Normalize a CORS origin to `*` or lowercased `scheme://authority`.
pub fn normalize_cors_origin(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed == "*" {
        return Ok("*".into());
    }
    let cleaned = trimmed.strip_suffix('/').unwrap_or(trimmed);
    let lower = cleaned.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return Err("Origin must be '*' or a valid http(s)://host[:port] origin".into());
    };
    if rest.is_empty()
        || rest.bytes().any(|b| {
            b.is_ascii_whitespace() || b.is_ascii_control() || matches!(b, b'/' | b'?' | b'#')
        })
    {
        return Err("Origin must be '*' or a valid http(s)://host[:port] origin".into());
    }
    Ok(lower)
}

fn normalize_cors_list<I, S>(items: I) -> Result<Vec<String>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out: Vec<String> = Vec::new();
    for raw in items {
        let s = raw.as_ref().trim();
        if s.is_empty() {
            continue;
        }
        let norm = normalize_cors_origin(s)?;
        if !out.contains(&norm) {
            if out.len() >= MAX_CORS_ORIGINS {
                return Err(format!("Maximum {MAX_CORS_ORIGINS} CORS origins allowed"));
            }
            out.push(norm);
        }
    }
    Ok(out)
}

pub struct TokenIdentity {
    pub id: String,
    #[cfg_attr(not(test), expect(dead_code))]
    pub owner: String,
}

#[derive(Debug)]
pub enum TokenAuthError {
    Unauthorized(&'static str),
    Forbidden(&'static str),
}

#[derive(Clone)]
pub struct TokenStore {
    path: PathBuf,
    data: Arc<Mutex<StoreData>>,
    seal_pepper: Option<Vec<u8>>,
}

fn sha256_hex(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn derive_vault_keys(signing_secret_hex: &str, pepper: Option<&[u8]>) -> ([u8; 32], [u8; 32]) {
    let base = hex::decode(signing_secret_hex).unwrap_or_else(|_| vec![0u8; 32]);
    let master = if let Some(p) = pepper {
        let mut mac = HmacSha256::new_from_slice(p).expect("HMAC takes any key length");
        mac.update(&base);
        mac.finalize().into_bytes().to_vec()
    } else {
        base
    };
    let mut enc_mac = HmacSha256::new_from_slice(&master).expect("HMAC takes any key length");
    enc_mac.update(b"520pdf:token-vault:enc:v1");
    let enc_key: [u8; 32] = enc_mac.finalize().into_bytes().into();

    let mut tag_mac = HmacSha256::new_from_slice(&master).expect("HMAC takes any key length");
    tag_mac.update(b"520pdf:token-vault:mac:v1");
    let mac_key: [u8; 32] = tag_mac.finalize().into_bytes().into();

    (enc_key, mac_key)
}

fn xor_ctr_stream(enc_key: &[u8; 32], nonce: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for (block_idx, chunk) in data.chunks(32).enumerate() {
        let mut h = HmacSha256::new_from_slice(enc_key).expect("HMAC takes any key length");
        h.update(nonce);
        h.update(&(block_idx as u32).to_be_bytes());
        let mask = h.finalize().into_bytes();
        for (b, m) in chunk.iter().zip(mask.iter()) {
            out.push(b ^ m);
        }
    }
    out
}

fn compute_vault_tag(
    mac_key: &[u8; 32],
    nonce: &[u8; 16],
    id: &str,
    owner: &str,
    ciphertext: &[u8],
) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(mac_key).expect("HMAC takes any key length");
    mac.update(b"v1|");
    mac.update(nonce);
    mac.update(b"|");
    mac.update(&(id.len() as u32).to_be_bytes());
    mac.update(id.as_bytes());
    mac.update(b"|");
    mac.update(&(owner.len() as u32).to_be_bytes());
    mac.update(owner.as_bytes());
    mac.update(b"|");
    mac.update(ciphertext);
    mac.finalize().into_bytes().into()
}

fn seal_token_secret(
    signing_secret_hex: &str,
    pepper: Option<&[u8]>,
    id: &str,
    owner: &str,
    plaintext: &str,
) -> String {
    let (enc_key, mac_key) = derive_vault_keys(signing_secret_hex, pepper);
    let mut nonce = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = xor_ctr_stream(&enc_key, &nonce, plaintext.as_bytes());
    let tag = compute_vault_tag(&mac_key, &nonce, id, owner, &ciphertext);
    format!(
        "v1:{}:{}:{}",
        hex::encode(nonce),
        hex::encode(ciphertext),
        hex::encode(tag)
    )
}

fn unseal_token_secret(
    signing_secret_hex: &str,
    pepper: Option<&[u8]>,
    id: &str,
    owner: &str,
    blob: &str,
) -> Option<String> {
    let mut parts = blob.split(':');
    let version = parts.next()?;
    let nonce_hex = parts.next()?;
    let ct_hex = parts.next()?;
    let tag_hex = parts.next()?;
    if version != "v1" || parts.next().is_some() {
        return None;
    }
    let nonce_vec = hex::decode(nonce_hex).ok()?;
    let nonce: [u8; 16] = nonce_vec.try_into().ok()?;
    let ciphertext = hex::decode(ct_hex).ok()?;
    let actual_tag = hex::decode(tag_hex).ok()?;

    let (enc_key, mac_key) = derive_vault_keys(signing_secret_hex, pepper);
    let expected_tag = compute_vault_tag(&mac_key, &nonce, id, owner, &ciphertext);
    if !constant_time_eq(&expected_tag, &actual_tag) {
        return None;
    }
    let plain_bytes = xor_ctr_stream(&enc_key, &nonce, &ciphertext);
    String::from_utf8(plain_bytes).ok()
}

impl TokenStore {
    pub fn load_or_init(dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&dir).map_err(|e| format!("create data dir: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        }
        let path = dir.join("store.json");
        let mut loaded: StoreData = if path.exists() {
            let raw = fs::read_to_string(&path).map_err(|e| format!("read store.json: {e}"))?;
            serde_json::from_str(&raw).map_err(|e| format!("parse store.json: {e}"))?
        } else {
            StoreData::default()
        };
        let mut dirty = false;
        if hex::decode(&loaded.signing_secret_hex)
            .map(|b| b.len() < 32)
            .unwrap_or(true)
        {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            loaded.signing_secret_hex = hex::encode(key);
            dirty = true;
        }
        let store = Self {
            path,
            data: Arc::new(Mutex::new(loaded)),
            seal_pepper: None,
        };
        if dirty || !store.path.exists() {
            let guard = store.data.lock().map_err(|_| "store lock poisoned")?;
            store.save_locked(&guard)?;
        }
        Ok(store)
    }

    pub fn set_seal_pepper(&mut self, pepper: &[u8]) {
        self.seal_pepper = Some(pepper.to_vec());
    }

    fn save_locked(&self, data: &StoreData) -> Result<(), String> {
        let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        let parent = self.path.parent().ok_or("missing store parent dir")?;
        let tmp = parent.join(format!("store.json.tmp.{}", std::process::id()));
        fs::write(&tmp, json.as_bytes()).map_err(|e| format!("write store tmp: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path).map_err(|e| format!("persist store.json: {e}"))
    }

    pub fn signing_secret(&self) -> Vec<u8> {
        let guard = self.data.lock().expect("store lock");
        hex::decode(&guard.signing_secret_hex).unwrap_or_else(|_| vec![0u8; 32])
    }

    pub fn has_any_users(&self) -> bool {
        self.data
            .lock()
            .map(|g| !g.users.is_empty())
            .unwrap_or(false)
    }

    pub fn has_user(&self, username: &str) -> bool {
        self.data
            .lock()
            .map(|g| g.users.iter().any(|u| u.username == username))
            .unwrap_or(false)
    }

    pub fn register_user(
        &self,
        raw_username: &str,
        password: &str,
        reserved_admin: Option<&str>,
    ) -> Result<StoredUser, String> {
        let username = raw_username.trim();
        if !(3..=40).contains(&username.len())
            || !username
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(
                "Username must be 3-40 ASCII letters, digits, dots, underscores, or hyphens".into(),
            );
        }
        if !(8..=128).contains(&password.len()) {
            return Err("Password must be 8-128 characters".into());
        }
        if reserved_admin.is_some_and(|a| !a.is_empty() && a.eq_ignore_ascii_case(username)) {
            return Err("Username is reserved".into());
        }
        let hash = hash_password(password)?;
        let user = StoredUser {
            username: username.to_owned(),
            password_hash: hash,
            created_at: now_iso(),
        };
        let mut guard = self.data.lock().map_err(|_| "store lock poisoned")?;
        if guard
            .users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(username))
        {
            return Err("Username already exists".into());
        }
        guard.users.push(user.clone());
        self.save_locked(&guard)?;
        Ok(user)
    }

    pub fn verify_stored_user(&self, raw_username: &str, password: &str) -> Option<String> {
        let username = raw_username.trim();
        let hash_and_name = {
            let guard = self.data.lock().ok()?;
            guard
                .users
                .iter()
                .find(|u| u.username.eq_ignore_ascii_case(username))
                .map(|u| (u.username.clone(), u.password_hash.clone()))
        };
        let (canonical_name, phc) = hash_and_name?;
        verify_password(password, &phc).then_some(canonical_name)
    }

    pub fn create_token(
        &self,
        owner: &str,
        raw_name: &str,
        raw_scopes: Vec<String>,
        expires_in_days: Option<u64>,
    ) -> Result<(TokenView, String), String> {
        let name = raw_name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err("Token name must be 1-64 characters".into());
        }
        let valid_tools = crate::http::workbench::all_tool_ids();
        let mut scopes = Vec::new();
        for s in raw_scopes {
            let s = s.trim();
            if s.is_empty() {
                continue;
            }
            if s == "*" {
                scopes.clear();
                scopes.push("*".into());
                break;
            }
            if !valid_tools.contains(&s) {
                return Err(format!("Unknown tool scope: {s}"));
            }
            if !scopes.iter().any(|existing| existing == s) {
                scopes.push(s.to_owned());
            }
        }
        if scopes.is_empty() {
            scopes.push("*".into());
        }

        let expires_at = match expires_in_days {
            Some(0) => return Err("expires_in_days must be at least 1".into()),
            Some(d) => Some(now_unix().saturating_add(d.clamp(1, 3650) * 86_400)),
            None => None,
        };

        let mut id_bytes = [0u8; 4];
        let mut secret_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut id_bytes);
        rand::thread_rng().fill_bytes(&mut secret_bytes);
        let id = format!("tok_{}", hex::encode(id_bytes));
        let raw_token = format!(
            "520pdf_k_{}_{}",
            hex::encode(id_bytes),
            hex::encode(secret_bytes)
        );
        let prefix = format!("520pdf_k_{}...", hex::encode(id_bytes));
        let token_hash = sha256_hex(&raw_token);

        let mut guard = self.data.lock().map_err(|_| "store lock poisoned")?;
        let active_count = guard
            .tokens
            .iter()
            .filter(|t| t.owner == owner && !t.revoked)
            .count();
        if active_count >= MAX_TOKENS_PER_USER {
            return Err(format!(
                "Maximum {MAX_TOKENS_PER_USER} active API tokens per user"
            ));
        }
        let encrypted_token = Some(seal_token_secret(
            &guard.signing_secret_hex,
            self.seal_pepper.as_deref(),
            &id,
            owner,
            &raw_token,
        ));
        let record = StoredToken {
            id,
            owner: owner.to_owned(),
            name: name.to_owned(),
            prefix,
            token_hash,
            encrypted_token,
            scopes,
            created_at: now_iso(),
            expires_at,
            last_used_at: None,
            usage_count: 0,
            revoked: false,
        };
        let view = TokenView::from(&record);
        guard.tokens.push(record);
        self.save_locked(&guard)?;
        Ok((view, raw_token))
    }

    pub fn list_tokens(&self, owner: &str) -> Vec<TokenView> {
        let Ok(guard) = self.data.lock() else {
            return Vec::new();
        };
        guard
            .tokens
            .iter()
            .rev()
            .filter(|t| t.owner == owner)
            .map(TokenView::from)
            .collect()
    }

    pub fn revoke_token(&self, owner: &str, id: &str) -> Result<(), String> {
        let mut guard = self.data.lock().map_err(|_| "store lock poisoned")?;
        let Some(token) = guard
            .tokens
            .iter_mut()
            .find(|t| t.id == id && t.owner == owner)
        else {
            return Err("API token not found".into());
        };
        token.revoked = true;
        token.encrypted_token = None;
        self.save_locked(&guard)
    }

    pub fn reveal_token(
        &self,
        owner: &str,
        id: &str,
    ) -> Result<(String, String), (StatusCode, &'static str)> {
        let guard = self
            .data
            .lock()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token store unavailable"))?;
        let Some(token) = guard.tokens.iter().find(|t| t.id == id && t.owner == owner) else {
            return Err((StatusCode::NOT_FOUND, "API token not found"));
        };
        if token.revoked {
            return Err((
                StatusCode::BAD_REQUEST,
                "Revoked API tokens cannot be viewed",
            ));
        }
        let Some(ref sealed) = token.encrypted_token else {
            return Err((
                StatusCode::CONFLICT,
                "This token was created before token viewing was enabled and cannot be revealed",
            ));
        };
        let Some(plaintext) = unseal_token_secret(
            &guard.signing_secret_hex,
            self.seal_pepper.as_deref(),
            &token.id,
            owner,
            sealed,
        ) else {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot decrypt API token",
            ));
        };
        if !constant_time_eq(
            sha256_hex(&plaintext).as_bytes(),
            token.token_hash.as_bytes(),
        ) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot decrypt API token",
            ));
        }
        Ok((token.name.clone(), plaintext))
    }

    pub fn authenticate_token(
        &self,
        raw_token: &str,
        tool: &str,
    ) -> Result<TokenIdentity, TokenAuthError> {
        let trimmed = raw_token.trim();
        if trimmed.is_empty() || trimmed.len() > 256 || !trimmed.starts_with("520pdf_k_") {
            return Err(TokenAuthError::Unauthorized("Invalid API key"));
        }
        let cand_hash = sha256_hex(trimmed);
        let mut guard = self
            .data
            .lock()
            .map_err(|_| TokenAuthError::Unauthorized("Token store unavailable"))?;

        let mut matched_idx: Option<usize> = None;
        for (i, t) in guard.tokens.iter().enumerate() {
            if constant_time_eq(t.token_hash.as_bytes(), cand_hash.as_bytes()) {
                matched_idx = Some(i);
            }
        }
        let Some(idx) = matched_idx else {
            return Err(TokenAuthError::Unauthorized("Invalid API key"));
        };

        let now = now_unix();
        let token = &mut guard.tokens[idx];
        if token.revoked {
            return Err(TokenAuthError::Unauthorized("API key has been revoked"));
        }
        if token.expires_at.is_some_and(|exp| now >= exp) {
            return Err(TokenAuthError::Unauthorized("API key has expired"));
        }
        if !token.scopes.iter().any(|s| s == "*" || s == tool) {
            return Err(TokenAuthError::Forbidden(
                "API key scope does not permit this tool",
            ));
        }
        token.usage_count = token.usage_count.saturating_add(1);
        token.last_used_at = Some(now_iso());
        let identity = TokenIdentity {
            id: token.id.clone(),
            owner: token.owner.clone(),
        };
        let _ = self.save_locked(&guard);
        Ok(identity)
    }

    /// Seed initial CORS origins from `CORS_ORIGINS` / `CORS_ORIGIN` if store has none yet.
    pub fn seed_cors_origins_from_env(&self) -> Result<(), String> {
        let raw = std::env::var("CORS_ORIGINS")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                std::env::var("CORS_ORIGIN")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            });
        let Some(raw) = raw else {
            return Ok(());
        };
        let list = normalize_cors_list(raw.split(','))?;
        let mut guard = self.data.lock().map_err(|_| "store lock poisoned")?;
        if guard.cors_origins.is_none() {
            guard.cors_origins = Some(list);
        }
        Ok(())
    }

    pub fn cors_origins(&self) -> Vec<String> {
        self.data
            .lock()
            .ok()
            .and_then(|g| g.cors_origins.clone())
            .unwrap_or_else(|| vec!["*".into()])
    }

    pub fn set_cors_origins(&self, raw_list: Vec<String>) -> Result<Vec<String>, String> {
        let clean = normalize_cors_list(raw_list)?;
        let mut guard = self.data.lock().map_err(|_| "store lock poisoned")?;
        guard.cors_origins = Some(clean.clone());
        self.save_locked(&guard)?;
        Ok(clean)
    }

    pub fn match_cors_origin(&self, origin: &str) -> CorsMatch {
        let Ok(norm) = normalize_cors_origin(origin) else {
            return CorsMatch::Denied;
        };
        if norm == "*" {
            return CorsMatch::Denied;
        }
        let origins = self.cors_origins();
        if origins.iter().any(|o| o == &norm) {
            CorsMatch::Explicit
        } else if origins.iter().any(|o| o == "*") {
            CorsMatch::Wildcard
        } else {
            CorsMatch::Denied
        }
    }
}

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_in_days: Option<u64>,
}

pub async fn list_tokens_handler(
    State(auth): State<Arc<AuthConfig>>,
    headers: HeaderMap,
) -> Response {
    let Some(user) = auth.current_user(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sign in to manage API keys"})),
        )
            .into_response();
    };
    let tokens = auth.store().list_tokens(&user);
    (StatusCode::OK, Json(json!({ "tokens": tokens }))).into_response()
}

pub async fn create_token_handler(
    State(auth): State<Arc<AuthConfig>>,
    headers: HeaderMap,
    payload: Result<Json<CreateTokenRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(user) = auth.current_user(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sign in to create API keys"})),
        )
            .into_response();
    };
    let Ok(Json(req)) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Provide JSON { name, scopes?, expires_in_days? }"})),
        )
            .into_response();
    };
    match auth
        .store()
        .create_token(&user, &req.name, req.scopes, req.expires_in_days)
    {
        Ok((meta, plaintext)) => (
            StatusCode::CREATED,
            Json(json!({
                "token": plaintext,
                "meta": meta,
            })),
        )
            .into_response(),
        Err(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
    }
}

pub async fn revoke_token_handler(
    State(auth): State<Arc<AuthConfig>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(user) = auth.current_user(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sign in to revoke API keys"})),
        )
            .into_response();
    };
    match auth.store().revoke_token(&user, &id) {
        Ok(()) => (StatusCode::OK, Json(json!({ "revoked": true, "id": id }))).into_response(),
        Err(msg) => (StatusCode::NOT_FOUND, Json(json!({"error": msg}))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct RevealTokenRequest {
    pub password: String,
}

pub async fn reveal_token_handler(
    State(auth): State<Arc<AuthConfig>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<RevealTokenRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(user) = auth.current_user(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sign in to view API keys"})),
        )
            .into_response();
    };
    let Ok(Json(req)) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Enter your login password"})),
        )
            .into_response();
    };
    if req.password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Enter your login password"})),
        )
            .into_response();
    }
    if !auth.verify_user_password(&user, &req.password) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Incorrect password"})),
        )
            .into_response();
    }
    match auth.store().reveal_token(&user, &id) {
        Ok((name, plaintext)) => {
            let mut res = (
                StatusCode::OK,
                Json(json!({
                    "id": id,
                    "name": name,
                    "token": plaintext,
                })),
            )
                .into_response();
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
        Err((status, msg)) => (status, Json(json!({"error": msg}))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateCorsRequest {
    pub origins: Vec<String>,
}

pub async fn get_cors_handler(State(auth): State<Arc<AuthConfig>>, headers: HeaderMap) -> Response {
    if auth.current_user(&headers).is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sign in to manage API CORS settings"})),
        )
            .into_response();
    }
    let origins = auth.store().cors_origins();
    let allow_all = origins.iter().any(|o| o == "*");
    let mut res = (
        StatusCode::OK,
        Json(json!({
            "origins": origins,
            "allow_all": allow_all,
        })),
    )
        .into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

pub async fn update_cors_handler(
    State(auth): State<Arc<AuthConfig>>,
    headers: HeaderMap,
    payload: Result<Json<UpdateCorsRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if auth.current_user(&headers).is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sign in to manage API CORS settings"})),
        )
            .into_response();
    }
    let Ok(Json(body)) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Provide JSON { origins: string[] }"})),
        )
            .into_response();
    };
    match auth.store().set_cors_origins(body.origins) {
        Ok(saved) => {
            let allow_all = saved.iter().any(|o| o == "*");
            let mut res = (
                StatusCode::OK,
                Json(json!({
                    "origins": saved,
                    "allow_all": allow_all,
                })),
            )
                .into_response();
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
        Err(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_login_create_verify_scope_and_revoke_token_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::load_or_init(dir.path().to_path_buf()).unwrap();

        let user = store
            .register_user("alice", "StrongPass-520!", Some("admin"))
            .unwrap();
        assert_eq!(user.username, "alice");
        assert!(user.password_hash.starts_with("$argon2id$v=19$"));
        assert!(
            store
                .register_user("admin", "StrongPass-520!", Some("admin"))
                .is_err()
        );
        assert!(
            store
                .verify_stored_user("ALICE", "StrongPass-520!")
                .as_deref()
                == Some("alice")
        );
        assert!(store.verify_stored_user("alice", "wrong-pass").is_none());

        let (meta, raw_secret) = store
            .create_token(
                "alice",
                "CI Pipeline",
                vec!["compress".into(), "merge".into()],
                Some(30),
            )
            .unwrap();
        assert!(raw_secret.starts_with("520pdf_k_"));
        assert!(!meta.revoked);
        assert!(meta.revealable);

        let disk_raw = fs::read_to_string(dir.path().join("store.json")).unwrap();
        assert!(
            !disk_raw.contains(&raw_secret),
            "plaintext secret must never hit disk"
        );

        let (rev_name, rev_secret) = store.reveal_token("alice", &meta.id).unwrap();
        assert_eq!(rev_name, "CI Pipeline");
        assert_eq!(rev_secret, raw_secret);

        let wrong_owner = store.reveal_token("bob", &meta.id);
        assert!(matches!(wrong_owner, Err((StatusCode::NOT_FOUND, _))));

        let ident = store.authenticate_token(&raw_secret, "compress").unwrap();
        assert_eq!(ident.owner, "alice");
        assert_eq!(ident.id, meta.id);

        let list = store.list_tokens("alice");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].usage_count, 1);
        assert!(list[0].revealable);

        assert!(matches!(
            store.authenticate_token(&raw_secret, "split"),
            Err(TokenAuthError::Forbidden(_))
        ));

        store.revoke_token("alice", &meta.id).unwrap();
        assert!(matches!(
            store.authenticate_token(&raw_secret, "compress"),
            Err(TokenAuthError::Unauthorized(_))
        ));
        assert!(!store.list_tokens("alice")[0].revealable);
        assert!(matches!(
            store.reveal_token("alice", &meta.id),
            Err((StatusCode::BAD_REQUEST, _))
        ));
    }

    #[test]
    fn tampered_token_ciphertext_fails_mac_verification() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::load_or_init(dir.path().to_path_buf()).unwrap();
        let (meta, _) = store
            .create_token("alice", "Tamper Test", vec!["*".into()], None)
            .unwrap();
        {
            let mut guard = store.data.lock().unwrap();
            let t = guard.tokens.iter_mut().find(|t| t.id == meta.id).unwrap();
            let mut blob = t.encrypted_token.clone().unwrap();
            let last = blob.pop().unwrap();
            blob.push(if last == '0' { '1' } else { '0' });
            t.encrypted_token = Some(blob);
        }
        assert!(matches!(
            store.reveal_token("alice", &meta.id),
            Err((StatusCode::INTERNAL_SERVER_ERROR, _))
        ));
    }

    #[test]
    fn expired_tokens_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::load_or_init(dir.path().to_path_buf()).unwrap();
        let (meta, raw_secret) = store
            .create_token("bob", "Temp", vec!["*".into()], Some(1))
            .unwrap();
        {
            let mut guard = store.data.lock().unwrap();
            let t = guard.tokens.iter_mut().find(|t| t.id == meta.id).unwrap();
            t.expires_at = Some(1);
        }
        assert!(matches!(
            store.authenticate_token(&raw_secret, "compress"),
            Err(TokenAuthError::Unauthorized(_))
        ));
    }

    #[test]
    fn cors_origins_default_wildcard_update_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::load_or_init(dir.path().to_path_buf()).unwrap();

        assert_eq!(store.cors_origins(), vec!["*".to_string()]);
        assert_eq!(
            store.match_cors_origin("https://AnySite.example/"),
            CorsMatch::Wildcard
        );

        let updated = store
            .set_cors_origins(vec![
                "https://App.Example.com/".into(),
                "http://localhost:3000".into(),
                "https://app.example.com".into(),
            ])
            .unwrap();
        assert_eq!(
            updated,
            vec![
                "https://app.example.com".to_string(),
                "http://localhost:3000".to_string(),
            ]
        );
        assert_eq!(
            store.match_cors_origin("https://app.example.com"),
            CorsMatch::Explicit
        );
        assert_eq!(
            store.match_cors_origin("https://blocked.example"),
            CorsMatch::Denied
        );

        assert!(
            store
                .set_cors_origins(vec!["ftp://bad.example".into()])
                .is_err()
        );
        assert!(
            store
                .set_cors_origins(vec!["https://bad.example/path".into()])
                .is_err()
        );
        assert!(store.set_cors_origins(vec!["not a url".into()]).is_err());

        // Persisted across reload
        let reloaded = TokenStore::load_or_init(dir.path().to_path_buf()).unwrap();
        assert_eq!(reloaded.cors_origins(), updated);
    }
}
