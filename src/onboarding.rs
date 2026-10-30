use anyhow::{Context, Result};
use serde_json::json;

use crate::{api::Api, declaration::Owner, flow};

pub fn authenticate(api: &mut Api, owner: &Owner, password: &str) -> Result<()> {
    let steps = api
        .get("/api/onboarding")?
        .expect_success("onboarding status")?;
    let done = |name: &str| {
        steps
            .as_array()
            .into_iter()
            .flatten()
            .any(|step| step["step"] == name && step["done"] == true)
    };
    let client_id = format!("{}/", api.base());
    let created_owner = !done("user");

    let code = if created_owner {
        log::info!("creating owner {}", owner.username);
        let created = api
            .post(
                "/api/onboarding/users",
                &json!({
                    "name": owner.name,
                    "username": owner.username,
                    "password": password,
                    "client_id": client_id,
                    "language": owner.language,
                }),
            )?
            .expect_success("creating owner")?;
        created["auth_code"]
            .as_str()
            .context("onboarding did not return an auth code")?
            .to_string()
    } else {
        let code = flow::drive(
            api,
            "/auth/login_flow",
            json!({ "client_id": client_id, "handler": ["homeassistant", null], "redirect_uri": client_id }),
            &[json!({ "username": owner.username, "password": password, "client_id": client_id })],
            "login",
        )?;
        code.as_str()
            .context("login flow did not return an auth code")?
            .to_string()
    };

    let token = api
        .post_form(
            "/auth/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("client_id", &client_id),
            ],
        )?
        .expect_success("token exchange")?;
    api.set_token(
        token["access_token"]
            .as_str()
            .context("token exchange did not return an access token")?
            .to_string(),
    );

    for step in ["core_config", "analytics"] {
        if !done(step) {
            log::info!("completing onboarding step {step}");
            api.post(&format!("/api/onboarding/{step}"), &json!({}))?
                .expect_success(step)?;
        }
    }
    if !done("integration") {
        log::info!("completing onboarding step integration");
        api.post(
            "/api/onboarding/integration",
            &json!({ "client_id": client_id, "redirect_uri": client_id }),
        )?
        .expect_success("integration")?;
    }
    Ok(())
}
