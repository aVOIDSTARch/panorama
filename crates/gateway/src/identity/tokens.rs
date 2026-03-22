use crate::error::GatewayApiError;
use crate::types::CallerIdentity;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use uuid::Uuid;

pub struct TokenStore {
    conn: Mutex<Connection>,
}

impl TokenStore {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    /// Issue a new caller token. Returns the plaintext token (shown once, never stored).
    pub fn issue(
        &self,
        caller_id: &str,
        allowed_routes: &[String],
        expires_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<String> {
        let token = Uuid::new_v4().to_string();
        let hash = hash_token(&token);
        let routes_json = serde_json::to_string(allowed_routes)?;
        let expires = expires_at.map(|dt| dt.to_rfc3339());

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO caller_tokens (caller_id, token_hash, issued_at, expires_at, allowed_routes, active)
             VALUES (?1, ?2, datetime('now'), ?3, ?4, 1)",
            rusqlite::params![caller_id, hash, expires, routes_json],
        )?;

        Ok(token)
    }

    /// Validate a bearer token. Returns the caller identity or an error.
    pub fn validate(&self, bearer: &str) -> Result<CallerIdentity, GatewayApiError> {
        let hash = hash_token(bearer);
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT caller_id, token_hash, issued_at, expires_at, allowed_routes, active
             FROM caller_tokens WHERE token_hash = ?1",
            [&hash],
            |row| {
                Ok(TokenRow {
                    caller_id: row.get(0)?,
                    token_hash: row.get(1)?,
                    issued_at: row.get(2)?,
                    expires_at: row.get(3)?,
                    allowed_routes: row.get(4)?,
                    active: row.get(5)?,
                })
            },
        );

        match result {
            Ok(row) => {
                if !row.active {
                    return Err(GatewayApiError::Unauthorized("token revoked".into()));
                }

                // Check expiry
                if let Some(ref expires_str) = row.expires_at {
                    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                        if expires < Utc::now() {
                            return Err(GatewayApiError::Unauthorized("token expired".into()));
                        }
                    }
                }

                let allowed_routes: Vec<String> =
                    serde_json::from_str(&row.allowed_routes).unwrap_or_default();

                let issued_at = chrono::DateTime::parse_from_rfc3339(&row.issued_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let expires_at = row.expires_at.as_ref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });

                Ok(CallerIdentity {
                    caller_id: row.caller_id,
                    token_hash: row.token_hash,
                    issued_at,
                    expires_at,
                    allowed_routes,
                    active: row.active,
                })
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(GatewayApiError::Unauthorized("invalid token".into()))
            }
            Err(e) => Err(GatewayApiError::Internal(format!("token lookup failed: {e}"))),
        }
    }

    /// Check if a caller is allowed to use a specific route.
    pub fn check_route_access(identity: &CallerIdentity, route_key: &str) -> bool {
        if identity.allowed_routes.is_empty() {
            return true;
        }
        identity
            .allowed_routes
            .iter()
            .any(|r| r == "*" || r == route_key)
    }

    pub fn revoke(&self, caller_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE caller_tokens SET active = 0 WHERE caller_id = ?1",
            [caller_id],
        )?;
        Ok(())
    }

    pub fn list(&self) -> anyhow::Result<Vec<CallerIdentity>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT caller_id, token_hash, issued_at, expires_at, allowed_routes, active
             FROM caller_tokens ORDER BY caller_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TokenRow {
                caller_id: row.get(0)?,
                token_hash: row.get(1)?,
                issued_at: row.get(2)?,
                expires_at: row.get(3)?,
                allowed_routes: row.get(4)?,
                active: row.get(5)?,
            })
        })?;

        let mut identities = Vec::new();
        for row in rows {
            let row = row?;
            let allowed_routes: Vec<String> =
                serde_json::from_str(&row.allowed_routes).unwrap_or_default();
            let issued_at = chrono::DateTime::parse_from_rfc3339(&row.issued_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let expires_at = row.expires_at.as_ref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            identities.push(CallerIdentity {
                caller_id: row.caller_id,
                token_hash: row.token_hash,
                issued_at,
                expires_at,
                allowed_routes,
                active: row.active,
            });
        }
        Ok(identities)
    }
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

struct TokenRow {
    caller_id: String,
    token_hash: String,
    issued_at: String,
    expires_at: Option<String>,
    allowed_routes: String,
    active: bool,
}

// We use a tiny inline hex encoder to avoid adding the `hex` crate
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .fold(String::new(), |mut s, b| {
                use std::fmt::Write;
                write!(s, "{b:02x}").unwrap();
                s
            })
    }
}
