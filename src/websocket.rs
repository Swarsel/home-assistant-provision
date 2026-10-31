use std::net::TcpStream;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

pub struct Ws {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl Ws {
    pub fn connect(base: &str, token: &str) -> Result<Ws> {
        let url = format!(
            "ws{}/api/websocket",
            base.trim_end_matches('/').trim_start_matches("http")
        );
        let (socket, _) =
            tungstenite::connect(url.as_str()).with_context(|| format!("connecting to {url}"))?;
        let mut ws = Ws { socket, next_id: 0 };
        let greeting = ws.receive()?;
        if greeting["type"] != "auth_required" {
            bail!("unexpected websocket greeting: {greeting}");
        }
        ws.send(&json!({ "type": "auth", "access_token": token }))?;
        let reply = ws.receive()?;
        if reply["type"] != "auth_ok" {
            bail!("websocket authentication failed: {reply}");
        }
        Ok(ws)
    }

    pub fn call(&mut self, kind: &str, mut payload: Map<String, Value>) -> Result<Value> {
        self.next_id += 1;
        payload.insert("id".to_string(), json!(self.next_id));
        payload.insert("type".to_string(), json!(kind));
        self.send(&Value::Object(payload))?;
        loop {
            let message = self.receive()?;
            if message["id"] != self.next_id {
                continue;
            }
            if message["success"] == true {
                return Ok(message["result"].clone());
            }
            bail!("{kind}: {}", message["error"]);
        }
    }

    pub fn close(mut self) {
        let _ = self.socket.close(None);
    }

    fn send(&mut self, value: &Value) -> Result<()> {
        self.socket.send(Message::Text(value.to_string().into()))?;
        Ok(())
    }

    fn receive(&mut self) -> Result<Value> {
        loop {
            match self.socket.read()? {
                Message::Text(text) => return Ok(serde_json::from_str(text.as_str())?),
                Message::Close(_) => bail!("websocket closed by Home Assistant"),
                _ => continue,
            }
        }
    }
}
