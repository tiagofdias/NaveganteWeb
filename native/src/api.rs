use anyhow::{Result, ensure};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{io::Read, time::Duration};
pub struct Api {
    client: Client,
    tiptop: String,
    edge: String,
}
impl Api {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("navegante/2026.10.19 android;NaveganteWeb Rust PCSC interoperability")
                .build()?,
            tiptop: "https://lisbon.tiptop.spiws.net/tiptop/".into(),
            edge: "https://pcgi.tmlmobilidade.pt/app/edge/".into(),
        })
    }
    #[cfg(test)]
    pub fn mock(base: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(3))
                .no_proxy()
                .build()
                .unwrap(),
            tiptop: base.clone(),
            edge: base,
        }
    }
    fn request(&self, method: &str, url: &str, body: Option<Value>) -> Result<Value> {
        let mut request = self
            .client
            .request(method.parse()?, url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(value) = body {
            request = request.json(&value)
        } else if method == "POST" {
            request = request.body("")
        };
        let response=request.send().map_err(|_|anyhow::anyhow!("Não foi possível contactar o serviço. Se já pagaste, consulta a referência antes de repetir."))?;
        ensure!(
            response.status().is_success(),
            "O serviço oficial respondeu HTTP {}. Tenta consultar o estado mais tarde.",
            response.status().as_u16()
        );
        let mut raw = vec![];
        response.take(2_000_001).read_to_end(&mut raw)?;
        ensure!(raw.len() <= 2_000_000, "Resposta demasiado grande.");
        if raw.is_empty() {
            Ok(Value::Null)
        } else {
            Ok(serde_json::from_slice(&raw)?)
        }
    }
    pub fn edge(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        self.request(method, &format!("{}{path}", self.edge), body)
    }
    pub fn exchange(&self, payload: Value) -> Result<Value> {
        let result = self.request("POST", &self.tiptop, Some(payload))?;
        ensure!(
            result.get("error").is_none(),
            "O serviço de bilhética recusou o pedido."
        );
        ensure!(
            result["status"]
                .as_u64()
                .is_some_and(|s| (200..300).contains(&s)),
            "O serviço de bilhética respondeu {}.",
            result["status"]
        );
        match &result["body"] {
            Value::String(s) if !s.is_empty() => Ok(serde_json::from_str(s)?),
            Value::String(_) => Ok(Value::Null),
            v => Ok(v.clone()),
        }
    }
    pub fn gateway(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let mut request = json!({"method":method,"path":path,"headers":{"Accept":"application/json","Content-Type":"application/json"}});
        if let Some(b) = body {
            request["body"] = Value::String(serde_json::to_string(&b)?)
        }
        self.exchange(json!({"id":201,"version":1,"request":request}))
    }
}
