use crate::{
    api::Api,
    model::{self, rows, text},
    pcsc::Context,
    protocol,
};
use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
#[derive(Clone)]
pub enum Command {
    Probe(String),
    Read(String),
    Catalog,
    Product(String),
    Prepare(String),
    Checkout {
        email: String,
        phone: String,
        nif: String,
        amount: i64,
    },
    Payment,
    Load,
    Recover {
        sale: String,
        reader: String,
    },
}
pub struct Service {
    api: Api,
    reader: String,
    read_id: String,
    card: Value,
    catalog: Value,
    details: Value,
    product_id: String,
    quote: Value,
    quote_at: Option<Instant>,
    sale: Option<String>,
    state: String,
    load_request: Value,
    read_at: Option<Instant>,
}
impl Service {
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: Api::new()?,
            reader: String::new(),
            read_id: String::new(),
            card: Value::Null,
            catalog: Value::Null,
            details: Value::Null,
            product_id: String::new(),
            quote: Value::Null,
            quote_at: None,
            sale: None,
            state: String::new(),
            load_request: Value::Null,
            read_at: None,
        })
    }
    pub fn run(&mut self, command: Command) -> Result<Value> {
        match command {
            Command::Probe(preferred) => {
                let ctx = Context::new()?;
                let readers = ctx.readers()?;
                let chosen = if readers.contains(&preferred) {
                    preferred
                } else if readers.len() == 1 {
                    readers[0].clone()
                } else {
                    String::new()
                };
                let present = if chosen.is_empty() {
                    false
                } else {
                    ctx.present(&chosen)?
                };
                Ok(json!({"readers":readers,"reader":chosen,"present":present}))
            }
            Command::Read(reader) => {
                ensure!(
                    (self.sale.is_none() && self.state.is_empty())
                        || self.state == "loaded"
                        || self.state == "canceled"
                        || self.state == "refunded",
                    "Conclui ou recupera o pedido existente antes de ler outro cartão."
                );
                let ctx = Context::new()?;
                let mut card = ctx.connect(&reader)?;
                let (read_id, data) = protocol::read(&self.api, |c| card.transmit(c))?;
                self.reader = reader;
                self.read_id = read_id;
                self.card = data.clone();
                self.catalog = Value::Null;
                self.details = Value::Null;
                self.product_id.clear();
                self.quote = Value::Null;
                self.sale = None;
                self.state.clear();
                self.read_at = Some(Instant::now());
                Ok(data)
            }
            Command::Recover { sale, reader } => {
                ensure!(
                    self.sale.is_none() || self.state == "loaded",
                    "Já existe um pedido em curso."
                );
                let sale = protocol::id(&json!(sale))?;
                let status = self
                    .api
                    .edge("GET", &format!("sales/{sale}/status"), None)?;
                ensure!(status.is_object(), "Referência não reconhecida.");
                self.reader = reader;
                self.sale = Some(sale.clone());
                self.state = "pending".into();
                self.load_request = Value::Null;
                Ok(json!({"saleId":sale,"state":"pending"}))
            }
            Command::Payment => self.payment(),
            Command::Load => {
                let status = self.payment()?;
                if self.state == "loaded" {
                    return Ok(status);
                }
                ensure!(
                    status["canLoad"] == true,
                    "O serviço ainda não autorizou a gravação. Atualiza o estado."
                );
                let ctx = Context::new()?;
                let mut card = ctx.connect(&self.reader)?;
                self.state = "loading".into();
                let mut started = false;
                let result = protocol::load(
                    &self.api,
                    &self.load_request,
                    |c| card.transmit(c),
                    &mut started,
                );
                if let Err(e) = result {
                    self.state = if started { "uncertain" } else { "paid" }.into();
                    return Err(e);
                }
                self.state = "uncertain".into();
                let sale = self.sale.as_ref().unwrap();
                let verified = self
                    .api
                    .edge("GET", &format!("sales/{sale}/status"), None)?;
                ensure!(
                    verified["isLoaded"] == true,
                    "Gravação ainda por confirmar. Atualiza o estado sem repetir o pagamento."
                );
                self.state = "loaded".into();
                self.load_request = Value::Null;
                Ok(json!({"state":"loaded","saleId":sale}))
            }
            Command::Checkout {
                email,
                phone,
                nif,
                amount,
            } => {
                ensure!(
                    self.sale.is_none() && self.state.is_empty(),
                    "Já existe um pedido. Não repitas o pagamento."
                );
                ensure!(
                    self.quote_at
                        .is_some_and(|at| at.elapsed() < Duration::from_secs(300)),
                    "O preço expirou. Consulta novamente."
                );
                ensure!(
                    self.quote["amount"].as_i64() == Some(amount),
                    "O preço mudou. Confirma o valor atualizado."
                );
                validate_customer(&email, &phone, &nif, amount)?;
                let session = protocol::id(&self.card["sessionId"])?;
                self.state = "submitting".into();
                let body = json!({"customer":{"name":null,"email":email.trim(),"phone":if phone.is_empty(){Value::Null}else{json!(format!("351#{phone}"))},"fiscalNumber":if nif.is_empty(){Value::Null}else{json!(nif)},"address":null},"paymentMethod":if amount==0{Value::Null}else{json!("MBWAY")}});
                // A lost response must never silently enable another charge.
                let result =
                    self.api
                        .edge("POST", &format!("payments/checkout/{session}"), Some(body));
                self.state = "unknown".into();
                let sale = result?;
                let sale_id = protocol::id(&sale["saleId"])?;
                self.sale = Some(sale_id.clone());
                self.state = "pending".into();
                let actual = if amount == 0 && sale["payment"].is_null() {
                    Some(0)
                } else {
                    sale["payment"]["amount"].as_i64()
                };
                ensure!(
                    actual == Some(amount),
                    "O valor diverge do confirmado. Não aproves o pagamento; consulta a referência."
                );
                Ok(json!({"saleId":sale_id,"state":"pending","amount":amount}))
            }
            command => {
                ensure!(
                    !self.read_id.is_empty()
                        && self
                            .read_at
                            .is_some_and(|at| at.elapsed() < Duration::from_secs(900)),
                    "A leitura expirou. Volta a ler o cartão."
                );
                ensure!(
                    self.sale.is_none() && self.state.is_empty(),
                    "Conclui o pedido antes de escolher outro produto."
                );
                match command {
                    Command::Catalog => {
                        if self.catalog.is_null() {
                            self.catalog = self.api.gateway(
                                "GET",
                                &format!("products?read_id={}", self.read_id),
                                None,
                            )?;
                        }
                        Ok(self.catalog.clone())
                    }
                    Command::Product(pid) => {
                        let pid = protocol::id(&json!(pid))?;
                        let mut available = vec![];
                        model::products(&self.catalog, &mut available);
                        ensure!(
                            available.iter().any(|p| p["id"] == pid),
                            "Produto indisponível para este cartão."
                        );
                        self.details = self.api.gateway(
                            "GET",
                            &format!("products/{pid}?read_id={}", self.read_id),
                            None,
                        )?;
                        self.product_id = pid.clone();
                        self.quote = Value::Null;
                        let mut details = self.details.clone();
                        for price in details["prices"].as_array_mut().into_iter().flatten() {
                            price["blockedReason"] = json!(model::blocked(
                                &self.card,
                                &pid,
                                &self.details,
                                &price["id"],
                                model::today()
                            ));
                        }
                        Ok(details)
                    }
                    Command::Prepare(variant) => {
                        self.quote = Value::Null;
                        self.quote_at = None;
                        let variant = protocol::id(&json!(variant))?;
                        let pid = protocol::id(&json!(self.product_id))?;
                        protocol::id(&self.details["id"])?;
                        let prices: Vec<_> = rows(&self.details["prices"])
                            .iter()
                            .filter(|p| p["id"] == variant)
                            .collect();
                        ensure!(prices.len() == 1, "Seleciona um preço oficial.");
                        if let Some(reason) = model::blocked(
                            &self.card,
                            &pid,
                            &self.details,
                            &json!(variant),
                            model::today(),
                        ) {
                            bail!(reason)
                        }
                        let mut price = prices[0].clone();
                        let mut amount = price_amount(&price)?;
                        for key in ["net", "tax", "tax_factor", "quantity"] {
                            let n = text(&price[key])
                                .parse::<u64>()
                                .map_err(|_| anyhow::anyhow!("Parâmetros de preço inválidos."))?;
                            price[key] = json!(n);
                        }
                        price.as_object_mut().unwrap().retain(|k, value| {
                            matches!(
                                k.as_str(),
                                "id" | "description" | "net" | "tax" | "tax_factor" | "quantity"
                            ) && !value.is_null()
                        });
                        let mut fields = vec![];
                        for attr in rows(&self.details["attributes"]) {
                            let values: Vec<_> = rows(&attr["values"])
                                .iter()
                                .filter(|v| v["id"] == variant)
                                .collect();
                            ensure!(
                                values.len() == 1,
                                "Esta configuração ainda não é suportada."
                            );
                            fields.push(json!({"id":values[0]["id"],"value":values[0]["value"]}));
                        }
                        let result=self.api.gateway("PUT",&format!("products/{pid}?read_id={}",self.read_id),Some(json!({"id":self.details["id"],"attributes":fields,"prices":[price]})))
                            .map_err(|e|anyhow::anyhow!("Não foi possível confirmar o preço no serviço oficial. Nenhum pagamento foi iniciado. {e}"))?;
                        if result.is_object() {
                            let matched: Vec<_> = rows(&result["prices"])
                                .iter()
                                .filter(|p| p["id"] == variant)
                                .collect();
                            ensure!(
                                matched.len() == 1,
                                "O serviço alterou as opções. Consulta novamente."
                            );
                            amount = price_amount(matched[0])?;
                        }
                        self.quote = json!({"product":pid,"variant":variant,"name":self.details["name"],"amount":amount});
                        self.quote_at = Some(Instant::now());
                        Ok(self.quote.clone())
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    fn payment(&mut self) -> Result<Value> {
        let sale = self.sale.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Pedido sem referência. Consulta o apoio antes de repetir o pagamento.")
        })?;
        let status = self
            .api
            .edge("GET", &format!("sales/{sale}/status"), None)?;
        ensure!(status.is_object(), "Estado inválido.");
        if status["isLoaded"] == true {
            self.state = "loaded".into();
            return Ok(json!({"state":"loaded","saleId":sale,"canLoad":false}));
        }
        let confirmation = self
            .api
            .edge("POST", &format!("payments/confirmation/{sale}"), None)?;
        let state = match confirmation["status"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "paid" => "paid",
            "created" | "pending_payment" => "pending",
            "canceled" => "canceled",
            "refunded" => "refunded",
            "pending_refund" => "refund_pending",
            _ => "unknown",
        };
        if state == "paid" && confirmation["loadRequest"].is_object() {
            self.load_request = confirmation["loadRequest"].clone()
        }
        if self.state != "uncertain" {
            self.state = state.into()
        }
        Ok(
            json!({"state":self.state,"saleId":sale,"canLoad":state=="paid"&&self.load_request.is_object()&&self.state!="uncertain"}),
        )
    }
    pub fn snapshot(&self) -> Value {
        json!({"saleId":self.sale,"state":self.state})
    }
}
pub fn price_amount(price: &Value) -> Result<i64> {
    let net = text(&price["net"]);
    let tax = text(&price["tax"]);
    ensure!(
        !net.is_empty()
            && !tax.is_empty()
            && net.bytes().all(|b| b.is_ascii_digit())
            && tax.bytes().all(|b| b.is_ascii_digit()),
        "Preço oficial inválido."
    );
    let amount = net
        .parse::<i64>()?
        .checked_add(tax.parse()?)
        .ok_or_else(|| anyhow::anyhow!("Preço inválido."))?;
    ensure!((0..=100000).contains(&amount), "Preço fora dos limites.");
    Ok(amount)
}
fn validate_customer(email: &str, phone: &str, nif: &str, amount: i64) -> Result<()> {
    ensure!(
        email.len() <= 254
            && regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")?.is_match(email.trim()),
        "Indica um e-mail válido para o recibo."
    );
    ensure!(
        amount == 0
            || (phone.len() == 9
                && phone.starts_with('9')
                && phone.bytes().all(|c| c.is_ascii_digit())),
        "Indica nove dígitos do número MB WAY."
    );
    if !nif.is_empty() {
        ensure!(
            nif.len() == 9 && nif.bytes().all(|c| c.is_ascii_digit()),
            "O NIF deve ter nove dígitos."
        );
        let digits: Vec<u32> = nif.chars().map(|c| c.to_digit(10).unwrap()).collect();
        let sum: u32 = digits[..8]
            .iter()
            .enumerate()
            .map(|(i, d)| d * (9 - i as u32))
            .sum();
        let check = 11 - sum % 11;
        ensure!(
            digits[8] == if check >= 10 { 0 } else { check },
            "O NIF não é válido."
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_float_price() {
        assert!(price_amount(&json!({"net":40.0,"tax":0})).is_err());
        assert_eq!(price_amount(&json!({"net":"4000","tax":0})).unwrap(), 4000);
    }
    #[test]
    fn validates_payment_data() {
        assert!(validate_customer("a@b.pt", "912345678", "", 4000).is_ok());
        assert!(validate_customer("a@b.pt", "123", "", 4000).is_err());
        assert!(validate_customer("bad", "912345678", "", 4000).is_err());
        assert!(validate_customer("a@b.pt", "912345678", "123456780", 4000).is_err());
    }
    fn checkout() -> Command {
        Command::Checkout {
            email: "a@b.pt".into(),
            phone: "912345678".into(),
            nif: "".into(),
            amount: 4000,
        }
    }
    fn quoted(api: Api) -> Service {
        let mut s = Service::new().unwrap();
        s.api = api;
        s.card = json!({"sessionId":"test-session"});
        s.quote = json!({"amount":4000});
        s.quote_at = Some(Instant::now());
        s
    }
    #[test]
    fn duplicate_checkout_never_reaches_network() {
        let (api, worker) = crate::test_support::api(vec![
            json!({"saleId":"test-sale","payment":{"amount":4000}}),
        ]);
        let mut s = quoted(api);
        assert!(s.run(checkout()).is_ok());
        assert!(s.run(checkout()).is_err());
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("payments/checkout/test-session"));
        assert!(requests[0].contains("351#912345678"));
    }
    #[test]
    fn mismatched_amount_preserves_sale_and_blocks_repeat() {
        let (api, worker) = crate::test_support::api(vec![
            json!({"saleId":"test-sale","payment":{"amount":5000}}),
        ]);
        let mut s = quoted(api);
        assert!(s.run(checkout()).is_err());
        assert_eq!(s.sale.as_deref(), Some("test-sale"));
        assert!(s.run(checkout()).is_err());
        worker.join().unwrap();
    }
    #[test]
    fn missing_reference_blocks_repeat() {
        let (api, worker) = crate::test_support::api(vec![json!({"unexpected":true})]);
        let mut s = quoted(api);
        assert!(s.run(checkout()).is_err());
        assert_eq!(s.state, "unknown");
        assert!(s.run(checkout()).is_err());
        worker.join().unwrap();
    }
    #[test]
    fn new_read_cannot_clear_an_unknown_payment() {
        let mut s = quoted(Api::mock("http://127.0.0.1:1/".into()));
        s.state = "unknown".into();
        assert!(
            s.run(Command::Read("not-a-reader".into()))
                .unwrap_err()
                .to_string()
                .contains("pedido existente")
        );
        assert_eq!(s.state, "unknown");
    }
    #[test]
    fn uncertain_write_cannot_be_repeated() {
        let (api, worker) = crate::test_support::api(vec![
            json!({"isLoaded":false}),
            json!({"status":"paid","loadRequest":{"request64":"x"}}),
        ]);
        let mut s = quoted(api);
        s.sale = Some("test-sale".into());
        s.state = "uncertain".into();
        assert!(s.run(Command::Load).is_err());
        assert_eq!(s.state, "uncertain");
        worker.join().unwrap();
    }
    #[test]
    fn loaded_state_is_authoritative() {
        let (api, worker) = crate::test_support::api(vec![json!({"isLoaded":true})]);
        let mut s = quoted(api);
        s.sale = Some("test-sale".into());
        s.state = "uncertain".into();
        assert_eq!(s.run(Command::Payment).unwrap()["state"], "loaded");
        worker.join().unwrap();
    }
    #[test]
    fn prepare_uses_selected_value_id_and_normalized_official_price() {
        let (api, worker) = crate::test_support::api(vec![
            json!({"status":200,"body":{"prices":[{"id":"variant-nov","net":4000,"tax":0}]}}),
        ]);
        let mut s = quoted(api);
        s.read_id = "test-read".into();
        s.read_at = Some(Instant::now());
        s.product_id = "id-prod-navegante-metro".into();
        s.details = json!({"id":"internal-product-id","name":"Metropolitano","attributes":[{"id":"product_monthly","values":[{"id":"variant-nov","value":"11"}]}],"prices":[{"id":"variant-nov","description":null,"net":"4000","tax":"0","quantity":"1","tax_factor":"0","unused":"do-not-send"}]});
        assert_eq!(
            s.run(Command::Prepare("variant-nov".into())).unwrap()["amount"],
            4000
        );
        let requests = worker.join().unwrap();
        let envelope: Value =
            serde_json::from_str(requests[0].split_once('\n').unwrap().1).unwrap();
        let body: Value =
            serde_json::from_str(envelope["request"]["body"].as_str().unwrap()).unwrap();
        assert_eq!(
            envelope["request"]["path"],
            "products/id-prod-navegante-metro?read_id=test-read"
        );
        assert_eq!(body["id"], "internal-product-id");
        assert_eq!(
            body["attributes"][0],
            json!({"id":"variant-nov","value":"11"})
        );
        assert_eq!(body["prices"][0]["net"], 4000);
        assert!(body["prices"][0].get("unused").is_none());
        assert!(body["prices"][0].get("description").is_none());
    }
    #[test]
    fn zapping_review_preserves_catalog_id_and_does_not_pay() {
        let details = json!({"id":"internal-zapping-id","name":"Zapping","attributes":[{"id":"product_occasional","values":[{"id":"fifteen","value":"1500"}]}],"prices":[{"id":"fifteen","net":"1415","tax":"85","quantity":"1500","tax_factor":"600"}]});
        let (api, worker) = crate::test_support::api(vec![
            json!({"status":200,"body":details}),
            json!({"status":200,"body":{"prices":[{"id":"fifteen","net":"1415","tax":"85"}]}}),
        ]);
        let mut s = quoted(api);
        s.read_id = "test-read".into();
        s.read_at = Some(Instant::now());
        s.catalog = json!({"products":[{"id":"id-prod-zapping"}]});
        s.run(Command::Product("id-prod-zapping".into())).unwrap();
        let quote = s.run(Command::Prepare("fifteen".into())).unwrap();
        assert_eq!(quote["amount"], 1500);
        assert_eq!(quote["product"], "id-prod-zapping");
        assert!(s.sale.is_none());
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 2);
        let envelope: Value =
            serde_json::from_str(requests[1].split_once('\n').unwrap().1).unwrap();
        assert_eq!(
            envelope["request"]["path"],
            "products/id-prod-zapping?read_id=test-read"
        );
        let body: Value =
            serde_json::from_str(envelope["request"]["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["id"], "internal-zapping-id");
        assert_eq!(body["prices"][0]["quantity"], 1500);
    }
    #[test]
    fn failed_price_review_clears_previous_quote() {
        let (api, worker) = crate::test_support::api(vec![json!({"status":500,"body":null})]);
        let mut s = quoted(api);
        s.read_id = "test-read".into();
        s.read_at = Some(Instant::now());
        s.product_id = "id-prod-zapping".into();
        s.details = json!({"id":"internal-zapping-id","prices":[{"id":"fifteen","net":"1415","tax":"85","quantity":"1500","tax_factor":"600"}]});
        let error = s.run(Command::Prepare("fifteen".into())).unwrap_err();
        assert!(error.to_string().contains("Nenhum pagamento"));
        assert!(s.quote.is_null());
        assert!(s.quote_at.is_none());
        assert!(s.sale.is_none());
        worker.join().unwrap();
    }
}
