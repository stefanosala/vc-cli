use anyhow::{Context, Result, anyhow};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::normalize_base_url;

#[derive(Clone)]
pub struct VolvoClient {
    base_url: String,
    client: reqwest::Client,
    api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OAuthDiscovery {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl VolvoClient {
    pub fn new(base_url: &str, api_key: &str) -> Result<Self> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(anyhow!("API key cannot be empty"));
        }
        Ok(Self {
            base_url: normalize_base_url(base_url)?,
            client: reqwest::Client::builder()
                .user_agent(format!("vc-cli/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .context("failed to initialize HTTP client")?,
            api_key: api_key.to_owned(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn get_vehicle_list(&self, access_token: &str) -> Result<Value> {
        self.get_vehicle_json("/vehicles", access_token, "vehicle list")
            .await
    }

    pub async fn get_vehicle_details(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(&vehicle_path(vin, "/"), access_token, "vehicle details")
            .await
    }

    pub async fn get_windows_status(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/windows"),
            access_token,
            "windows status",
        )
        .await
    }

    pub async fn get_doors_status(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/doors"),
            access_token,
            "doors and lock status",
        )
        .await
    }

    pub async fn get_warnings(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(&vehicle_path(vin, "/warnings"), access_token, "warnings")
            .await
    }

    pub async fn get_tyre_pressure_values(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/tyres"),
            access_token,
            "tyre pressure values",
        )
        .await
    }

    pub async fn get_statistics(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/statistics"),
            access_token,
            "statistics",
        )
        .await
    }

    pub async fn get_odometer(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(&vehicle_path(vin, "/odometer"), access_token, "odometer")
            .await
    }

    pub async fn get_fuel_amount(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(&vehicle_path(vin, "/fuel"), access_token, "fuel amount")
            .await
    }

    pub async fn get_diagnostics(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/diagnostics"),
            access_token,
            "diagnostics",
        )
        .await
    }

    pub async fn get_engine_diagnostics(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/engine"),
            access_token,
            "engine diagnostics",
        )
        .await
    }

    pub async fn get_engine_status(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/engine-status"),
            access_token,
            "engine status",
        )
        .await
    }

    pub async fn get_brake_status(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(&vehicle_path(vin, "/brakes"), access_token, "brake status")
            .await
    }

    pub async fn get_energy_state(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_energy_json(&energy_path(vin, "/state"), access_token, "energy state")
            .await
    }

    pub async fn get_energy_capabilities(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_energy_json(
            &energy_path(vin, "/capabilities"),
            access_token,
            "energy capabilities",
        )
        .await
    }

    pub async fn get_vehicle_location(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_location_json(&location_path(vin), access_token, "latest vehicle location")
            .await
    }

    pub async fn get_command_list(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/commands"),
            access_token,
            "command list",
        )
        .await
    }

    pub async fn get_commands_accessibility(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.get_vehicle_json(
            &vehicle_path(vin, "/command-accessibility"),
            access_token,
            "commands accessibility",
        )
        .await
    }

    pub async fn invoke_unlock(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/unlock"),
            access_token,
            "unlock invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_lock(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/lock"),
            access_token,
            "lock invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_lock_reduced_guard(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/lock-reduced-guard"),
            access_token,
            "lock-reduced-guard invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_honk(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/honk"),
            access_token,
            "honk invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_flash(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/flash"),
            access_token,
            "flash invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_honk_flash(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/honk-flash"),
            access_token,
            "honk-flash invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_engine_start(
        &self,
        vin: &str,
        access_token: &str,
        request_body: Value,
    ) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/engine-start"),
            access_token,
            "engine-start invocation",
            request_body,
        )
        .await
    }

    pub async fn invoke_engine_stop(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/engine-stop"),
            access_token,
            "engine-stop invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_climatization_start(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/climatization-start"),
            access_token,
            "climatization-start invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    pub async fn invoke_climatization_stop(&self, vin: &str, access_token: &str) -> Result<Value> {
        self.post_vehicle_json(
            &vehicle_command_path(vin, "/climatization-stop"),
            access_token,
            "climatization-stop invocation",
            Value::Object(Default::default()),
        )
        .await
    }

    async fn get_vehicle_json(&self, path: &str, access_token: &str, label: &str) -> Result<Value> {
        let response = self
            .client
            .get(vehicle_url(&self.base_url, path))
            .header("accept", "application/json")
            .header("vcc-api-key", &self.api_key)
            .bearer_auth(access_token.trim())
            .send()
            .await
            .with_context(|| format!("failed to send {label} request"))?;
        ensure_success_json(response, label).await
    }

    async fn post_vehicle_json(
        &self,
        path: &str,
        access_token: &str,
        label: &str,
        body: Value,
    ) -> Result<Value> {
        let response = self
            .client
            .post(vehicle_url(&self.base_url, path))
            .header("accept", "application/json")
            .header("vcc-api-key", &self.api_key)
            .bearer_auth(access_token.trim())
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to send {label} request"))?;
        ensure_success_json(response, label).await
    }

    async fn get_energy_json(&self, path: &str, access_token: &str, label: &str) -> Result<Value> {
        let response = self
            .client
            .get(energy_url(&self.base_url, path))
            .header("accept", "application/json")
            .header("vcc-api-key", &self.api_key)
            .bearer_auth(access_token.trim())
            .send()
            .await
            .with_context(|| format!("failed to send {label} request"))?;
        ensure_success_json(response, label).await
    }

    async fn get_location_json(
        &self,
        path: &str,
        access_token: &str,
        label: &str,
    ) -> Result<Value> {
        let response = self
            .client
            .get(location_url(&self.base_url, path))
            .header("accept", "application/json")
            .header("vcc-api-key", &self.api_key)
            .bearer_auth(access_token.trim())
            .send()
            .await
            .with_context(|| format!("failed to send {label} request"))?;
        ensure_success_json(response, label).await
    }

    pub async fn fetch_discovery(issuer: &str, client: &reqwest::Client) -> Result<OAuthDiscovery> {
        let issuer = normalize_base_url(issuer)?;
        let response = client
            .get(format!("{issuer}/.well-known/openid-configuration"))
            .header("accept", "application/json")
            .send()
            .await
            .context("failed to fetch OIDC discovery document")?;
        let response = response
            .error_for_status()
            .context("OIDC discovery request was not successful")?;
        response
            .json::<OAuthDiscovery>()
            .await
            .context("failed to parse OIDC discovery response")
    }

    pub async fn exchange_authorization_code(
        client: &reqwest::Client,
        token_endpoint: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthTokenResponse> {
        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("code", code),
            ("code_verifier", code_verifier),
        ];
        let response = client
            .post(token_endpoint)
            .header("accept", "application/json")
            .form(&params)
            .send()
            .await
            .context("failed to exchange authorization code for token")?;
        parse_token_response(response).await
    }

    pub async fn refresh_access_token(
        client: &reqwest::Client,
        token_endpoint: &str,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse> {
        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ];
        let response = client
            .post(token_endpoint)
            .header("accept", "application/json")
            .form(&params)
            .send()
            .await
            .context("failed to refresh access token")?;
        parse_token_response(response).await
    }

    pub async fn refresh_access_token_bridge(
        client: &reqwest::Client,
        refresh_endpoint: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse> {
        let response = client
            .post(refresh_endpoint)
            .header("accept", "application/json")
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .context("failed to refresh access token via bridge")?;
        parse_token_response(response).await
    }
}

fn vehicle_path(vin: &str, suffix: &str) -> String {
    if suffix == "/" {
        return format!("/vehicles/{vin}");
    }
    format!("/vehicles/{vin}{suffix}")
}

fn vehicle_command_path(vin: &str, suffix: &str) -> String {
    format!("/vehicles/{vin}/commands{suffix}")
}

fn energy_path(vin: &str, suffix: &str) -> String {
    format!("/vehicles/{vin}{suffix}")
}

fn location_path(vin: &str) -> String {
    format!("/vehicles/{vin}/location")
}

fn vehicle_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/connected-vehicle/v2{path}")
}

fn energy_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/energy/v2{path}")
}

fn location_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/location/v1{path}")
}

async fn ensure_success_json(response: reqwest::Response, label: &str) -> Result<Value> {
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read {label} response body"))?;
    if status.is_success() {
        if body.trim().is_empty() {
            return Ok(Value::Null);
        }
        return serde_json::from_str::<Value>(&body)
            .with_context(|| format!("failed to parse {label} response as JSON"));
    }
    Err(anyhow!(
        "{label} request failed ({status}): {}",
        truncate_error_body(&body)
    ))
}

async fn parse_token_response(response: reqwest::Response) -> Result<OAuthTokenResponse> {
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read token endpoint response body")?;
    if status != StatusCode::OK {
        return Err(anyhow!(
            "token endpoint request failed ({status}): {}",
            truncate_error_body(&body)
        ));
    }
    serde_json::from_str::<OAuthTokenResponse>(&body)
        .context("failed to parse token endpoint response")
}

fn truncate_error_body(body: &str) -> String {
    const MAX: usize = 300;
    let trimmed = body.trim();
    if trimmed.len() <= MAX {
        return trimmed.to_owned();
    }
    format!("{}...", &trimmed[..MAX])
}

#[cfg(test)]
mod tests {
    use super::{energy_path, location_path, location_url, vehicle_command_path, vehicle_path};

    #[test]
    fn vehicle_paths_are_stable() {
        assert_eq!(vehicle_path("YV1AA1234", "/"), "/vehicles/YV1AA1234");
        assert_eq!(
            vehicle_path("YV1AA1234", "/windows"),
            "/vehicles/YV1AA1234/windows"
        );
        assert_eq!(
            vehicle_command_path("YV1AA1234", "/lock"),
            "/vehicles/YV1AA1234/commands/lock"
        );
        assert_eq!(
            energy_path("YV1AA1234", "/state"),
            "/vehicles/YV1AA1234/state"
        );
        assert_eq!(
            energy_path("YV1AA1234", "/capabilities"),
            "/vehicles/YV1AA1234/capabilities"
        );
        assert_eq!(location_path("YV1AA1234"), "/vehicles/YV1AA1234/location");
    }

    #[test]
    fn location_url_is_stable() {
        assert_eq!(
            location_url("https://api.volvocars.com", "/vehicles/YV1AA1234/location"),
            "https://api.volvocars.com/location/v1/vehicles/YV1AA1234/location"
        );
    }
}
