use anyhow::Result;
use serde::Serialize;

use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Serialize)]
pub struct AuthLogoutOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
}

pub fn execute(store: &Store, profile: &Profile, base_url: &str) -> Result<AuthLogoutOutput> {
    store.clear_auth_session(profile.id)?;
    Ok(AuthLogoutOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: base_url.to_owned(),
    })
}
