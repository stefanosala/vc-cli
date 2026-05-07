use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::store::sqlite::{Profile, Store, StoredVin};

#[derive(Debug, Serialize)]
pub struct AuthWhoAmIOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
    pub authenticated: bool,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
    pub default_vin: Option<String>,
    pub vins: Vec<StoredVin>,
}

pub fn execute(store: &Store, profile: &Profile, base_url: &str) -> Result<AuthWhoAmIOutput> {
    let session = store.get_auth_session(profile.id)?;
    if session.is_none() {
        return Err(anyhow!(
            "no auth session for this profile; run `vc-cli auth login` first"
        ));
    }
    let session = session.expect("checked above");
    let vins = store.list_vins(profile.id)?;
    let default_vin = vins
        .iter()
        .find(|vin| vin.is_default)
        .map(|vin| vin.vin.clone());
    Ok(AuthWhoAmIOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: base_url.to_owned(),
        authenticated: true,
        expires_at: session.expires_at,
        scope: session.scope,
        default_vin,
        vins,
    })
}
