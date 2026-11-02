use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use ureq::{Agent, Body, http};

pub struct Api {
    agent: Agent,
    base: String,
    token: Option<String>,
}

pub struct Response {
    pub status: u16,
    pub body: Value,
}

impl Response {
    pub fn expect_success(self, what: &str) -> Result<Value> {
        if self.status >= 400 {
            bail!("{what}: HTTP {}: {}", self.status, self.body);
        }
        Ok(self.body)
    }
}

impl Api {
    pub fn new(base: &str) -> Api {
        let config = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .build();
        Api {
            agent: config.into(),
            base: base.trim_end_matches('/').to_string(),
            token: None,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub fn token(&self) -> Result<&str> {
        self.token.as_deref().context("not authenticated")
    }

    pub fn get(&self, path: &str) -> Result<Response> {
        let mut request = self.agent.get(self.url(path));
        if let Some(value) = self.authorization() {
            request = request.header("Authorization", value);
        }
        read(request.call()?)
    }

    pub fn delete(&self, path: &str) -> Result<Response> {
        let mut request = self.agent.delete(self.url(path));
        if let Some(value) = self.authorization() {
            request = request.header("Authorization", value);
        }
        read(request.call()?)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Response> {
        let mut request = self.agent.post(self.url(path));
        if let Some(value) = self.authorization() {
            request = request.header("Authorization", value);
        }
        read(request.send_json(body)?)
    }

    pub fn post_form(&self, path: &str, fields: &[(&str, &str)]) -> Result<Response> {
        read(
            self.agent
                .post(self.url(path))
                .send_form(fields.iter().copied())?,
        )
    }

    pub fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            match self.get("/manifest.json") {
                Ok(response) if response.status == 200 => return Ok(()),
                Ok(response) => log::info!("waiting for Home Assistant (HTTP {})", response.status),
                Err(error) => log::info!("waiting for Home Assistant ({error})"),
            }
            if started.elapsed() > timeout {
                bail!(
                    "Home Assistant did not become ready within {} seconds",
                    timeout.as_secs()
                );
            }
            thread::sleep(Duration::from_secs(2));
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn authorization(&self) -> Option<String> {
        self.token.as_ref().map(|token| format!("Bearer {token}"))
    }
}

fn read(mut response: http::Response<Body>) -> Result<Response> {
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string()?;
    let body = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    };
    Ok(Response { status, body })
}
