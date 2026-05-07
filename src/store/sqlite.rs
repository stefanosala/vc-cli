use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub profile_id: i64,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub expires_at: Option<i64>,
    pub token_endpoint: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone)]
pub struct PersistedTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub expires_at: Option<i64>,
    pub token_endpoint: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVin {
    pub vin: String,
    pub is_default: bool,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create parent directories for sqlite path {}",
                    path.display()
                )
            })?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open sqlite database at {}", path.display()))?;
        conn.execute_batch(include_str!("migrations/0001_init.sql"))
            .context("failed to apply sqlite schema")?;
        Ok(Self { conn })
    }

    pub fn get_or_create_profile(&self, name: &str, fallback_base_url: &str) -> Result<Profile> {
        if let Some(profile) = self.get_profile_by_name(name)? {
            return Ok(profile);
        }
        self.conn
            .execute(
                "INSERT INTO profiles (name, base_url, is_active) VALUES (?1, ?2, 0)",
                params![name, fallback_base_url],
            )
            .context("failed to create profile")?;
        self.get_profile_by_name(name)?
            .ok_or_else(|| anyhow!("profile insertion failed unexpectedly"))
    }

    pub fn get_profile_by_name(&self, name: &str) -> Result<Option<Profile>> {
        self.conn
            .query_row(
                "SELECT id, name, base_url, is_active FROM profiles WHERE name = ?1",
                params![name],
                map_profile,
            )
            .optional()
            .context("failed to read profile by name")
    }

    pub fn get_active_profile(&self) -> Result<Option<Profile>> {
        self.conn
            .query_row(
                "SELECT id, name, base_url, is_active FROM profiles WHERE is_active = 1 LIMIT 1",
                [],
                map_profile,
            )
            .optional()
            .context("failed to read active profile")
    }

    pub fn set_active_profile(&self, profile_name: &str) -> Result<Profile> {
        let profile = self
            .get_profile_by_name(profile_name)?
            .ok_or_else(|| anyhow!("profile `{profile_name}` was not found"))?;
        self.conn
            .execute("UPDATE profiles SET is_active = 0", [])
            .context("failed to clear active profile marker")?;
        self.conn
            .execute(
                "UPDATE profiles SET is_active = 1 WHERE id = ?1",
                params![profile.id],
            )
            .context("failed to set active profile marker")?;
        self.get_active_profile()?
            .ok_or_else(|| anyhow!("active profile was not set"))
    }

    pub fn set_profile_base_url(&self, profile_id: i64, base_url: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE profiles SET base_url = ?1 WHERE id = ?2",
                params![base_url, profile_id],
            )
            .context("failed to update profile base URL")?;
        Ok(())
    }

    pub fn save_auth_session(&self, profile_id: i64, token_set: &PersistedTokenSet) -> Result<()> {
        let now = unix_now();
        self.conn
            .execute(
                "INSERT INTO auth_sessions (
                    profile_id, access_token, refresh_token, scope, token_type, expires_at,
                    token_endpoint, client_id, client_secret, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(profile_id) DO UPDATE SET
                    access_token=excluded.access_token,
                    refresh_token=excluded.refresh_token,
                    scope=excluded.scope,
                    token_type=excluded.token_type,
                    expires_at=excluded.expires_at,
                    token_endpoint=excluded.token_endpoint,
                    client_id=excluded.client_id,
                    client_secret=excluded.client_secret,
                    updated_at=excluded.updated_at",
                params![
                    profile_id,
                    token_set.access_token,
                    token_set.refresh_token,
                    token_set.scope,
                    token_set.token_type,
                    token_set.expires_at,
                    token_set.token_endpoint,
                    token_set.client_id,
                    token_set.client_secret,
                    now,
                ],
            )
            .context("failed to persist auth session")?;
        Ok(())
    }

    pub fn get_auth_session(&self, profile_id: i64) -> Result<Option<AuthSession>> {
        self.conn
            .query_row(
                "SELECT
                    profile_id, access_token, refresh_token, scope, token_type, expires_at,
                    token_endpoint, client_id, client_secret
                 FROM auth_sessions WHERE profile_id = ?1",
                params![profile_id],
                |row| {
                    Ok(AuthSession {
                        profile_id: row.get(0)?,
                        access_token: row.get(1)?,
                        refresh_token: row.get(2)?,
                        scope: row.get(3)?,
                        token_type: row.get(4)?,
                        expires_at: row.get(5)?,
                        token_endpoint: row.get(6)?,
                        client_id: row.get(7)?,
                        client_secret: row.get(8)?,
                    })
                },
            )
            .optional()
            .context("failed to load auth session")
    }

    pub fn clear_auth_session(&self, profile_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM auth_sessions WHERE profile_id = ?1",
                params![profile_id],
            )
            .context("failed to clear auth session")?;
        Ok(())
    }

    pub fn upsert_vin(&self, profile_id: i64, vin: &str, set_default: bool) -> Result<()> {
        let vin = normalize_vin(vin)?;
        if set_default {
            self.conn
                .execute(
                    "UPDATE profile_vins SET is_default = 0 WHERE profile_id = ?1",
                    params![profile_id],
                )
                .context("failed to clear previous default VIN")?;
        }
        self.conn
            .execute(
                "INSERT INTO profile_vins (profile_id, vin, is_default) VALUES (?1, ?2, ?3)
                 ON CONFLICT(profile_id, vin) DO UPDATE SET is_default=excluded.is_default",
                params![profile_id, vin, i64::from(set_default)],
            )
            .context("failed to save VIN")?;
        Ok(())
    }

    pub fn set_default_vin(&self, profile_id: i64, vin: &str) -> Result<()> {
        let vin = normalize_vin(vin)?;
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM profile_vins WHERE profile_id = ?1 AND vin = ?2",
                params![profile_id, vin],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read VIN by profile")?;
        if exists.is_none() {
            return Err(anyhow!(
                "VIN `{vin}` is not stored for the active profile; add it with `vehicle vin add --vin {vin}` first"
            ));
        }
        self.conn
            .execute(
                "UPDATE profile_vins SET is_default = 0 WHERE profile_id = ?1",
                params![profile_id],
            )
            .context("failed to clear default VIN")?;
        self.conn
            .execute(
                "UPDATE profile_vins SET is_default = 1 WHERE profile_id = ?1 AND vin = ?2",
                params![profile_id, vin],
            )
            .context("failed to set default VIN")?;
        Ok(())
    }

    pub fn list_vins(&self, profile_id: i64) -> Result<Vec<StoredVin>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT vin, is_default FROM profile_vins
                 WHERE profile_id = ?1 ORDER BY is_default DESC, vin ASC",
            )
            .context("failed to prepare VIN list query")?;
        let rows = stmt
            .query_map(params![profile_id], |row| {
                Ok(StoredVin {
                    vin: row.get(0)?,
                    is_default: row.get::<_, i64>(1)? == 1,
                })
            })
            .context("failed to query VIN rows")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to parse VIN rows")
    }

    pub fn resolve_vin(&self, profile_id: i64, explicit: Option<&str>) -> Result<String> {
        if let Some(vin) = explicit {
            return normalize_vin(vin);
        }
        let maybe_default: Option<String> = self
            .conn
            .query_row(
                "SELECT vin FROM profile_vins WHERE profile_id = ?1 AND is_default = 1 LIMIT 1",
                params![profile_id],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read default VIN")?;
        maybe_default.ok_or_else(|| {
            anyhow!(
                "no VIN provided and no default VIN configured; run `vehicle vin add --vin <VIN> --default` or `vehicle vin default --vin <VIN>`"
            )
        })
    }
}

fn map_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        is_active: row.get::<_, i64>(3)? == 1,
    })
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_secs() as i64
}

fn normalize_vin(vin: &str) -> Result<String> {
    let normalized = vin.trim().to_uppercase();
    if normalized.is_empty() {
        return Err(anyhow!("VIN cannot be empty"));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn default_vin_is_unique_per_profile() {
        let file = NamedTempFile::new().expect("temp file should create");
        let store = Store::open(file.path()).expect("store should open");
        let profile = store
            .get_or_create_profile("default", "https://api.volvocars.com")
            .expect("profile should create");
        store
            .upsert_vin(profile.id, "VIN1", true)
            .expect("vin1 should save");
        store
            .upsert_vin(profile.id, "VIN2", true)
            .expect("vin2 should save");
        let vins = store.list_vins(profile.id).expect("list vins should work");
        let defaults = vins.iter().filter(|vin| vin.is_default).count();
        assert_eq!(defaults, 1);
        assert_eq!(vins[0].vin, "VIN2");
    }

    #[test]
    fn resolve_vin_uses_default_when_not_explicit() {
        let file = NamedTempFile::new().expect("temp file should create");
        let store = Store::open(file.path()).expect("store should open");
        let profile = store
            .get_or_create_profile("default", "https://api.volvocars.com")
            .expect("profile should create");
        store
            .upsert_vin(profile.id, "YV1AA111111111111", true)
            .expect("default vin should save");
        let resolved = store
            .resolve_vin(profile.id, None)
            .expect("default VIN should resolve");
        assert_eq!(resolved, "YV1AA111111111111");
    }
}
