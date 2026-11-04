#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod api;
mod model;
mod pcsc;
mod protocol;
mod service;
#[cfg(test)]
mod test_support;
use eframe::egui::{self, Color32, RichText, Vec2};
use serde_json::{Value, json};
use service::Command;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
const YELLOW: Color32 = Color32::from_rgb(255, 224, 0);
const MUTED: Color32 = Color32::from_rgb(150, 163, 164);
#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Home,
    Detect,
    Reading,
    Summary,
    Catalog,
    Options,
    Checkout,
    Payment,
    Done,
}
struct Reply {
    kind: &'static str,
    result: Result<Value, String>,
    purchase: Value,
}
struct App {
    tx: Sender<Command>,
    rx: Receiver<Reply>,
    step: Step,
    busy: bool,
    error: String,
    readers: Vec<String>,
    reader: String,
    card: Value,
    products: Vec<Value>,
    details: Value,
    variant: String,
    quote: Value,
    purchase: Value,
    email: String,
    phone: String,
    nif: String,
    confirm_write: bool,
    last_probe: Instant,
    last_poll: Instant,
    recovery: String,
    recovering: bool,
    demo: bool,
    close_dialog: bool,
    allow_close: bool,
    show_other: bool,
    balance_only: bool,
    probe_failed: bool,
}
fn kind(c: &Command) -> &'static str {
    match c {
        Command::Probe(_) => "probe",
        Command::Read(_) => "read",
        Command::Catalog => "catalog",
        Command::Product(_) => "product",
        Command::Prepare(_) => "prepare",
        Command::Checkout { .. } => "checkout",
        Command::Payment => "payment",
        Command::Load => "load",
        Command::Recover { .. } => "recover",
    }
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>, demo: bool) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(12, 17, 18);
        visuals.window_fill = Color32::from_rgb(21, 28, 29);
        visuals.override_text_color = Some(Color32::from_rgb(240, 244, 239));
        visuals.selection.bg_fill = YELLOW.gamma_multiply(0.25);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(9);
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(9);
        visuals.widgets.active.corner_radius = egui::CornerRadius::same(9);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 39, 40);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 59, 60);
        cc.egui_ctx.set_visuals(visuals);
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(12., 12.);
        style.spacing.button_padding = Vec2::new(22., 14.);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(16.));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(30.));
        cc.egui_ctx.set_style(style);
        let (tx, commands) = mpsc::channel();
        let (replies, rx) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let mut backend = service::Service::new().map_err(|e| e.to_string());
            let mut demo_backend = Demo {
                wait: std::env::args().any(|a| a == "--demo-wait"),
                ..Default::default()
            };
            while let Ok(command) = commands.recv() {
                let kind = kind(&command);
                let result = if demo {
                    demo_backend.run(command)
                } else {
                    match &mut backend {
                        Ok(service) => service.run(command).map_err(|e| e.to_string()),
                        Err(e) => Err(e.clone()),
                    }
                };
                let purchase = if demo {
                    demo_backend.purchase.clone()
                } else {
                    backend
                        .as_ref()
                        .map(|s| s.snapshot())
                        .unwrap_or(Value::Null)
                };
                if replies
                    .send(Reply {
                        kind,
                        result,
                        purchase,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self {
            tx,
            rx,
            step: Step::Home,
            busy: false,
            error: String::new(),
            readers: vec![],
            reader: String::new(),
            card: Value::Null,
            products: vec![],
            details: Value::Null,
            variant: String::new(),
            quote: Value::Null,
            purchase: Value::Null,
            email: if demo {
                "demo@example.invalid".into()
            } else {
                String::new()
            },
            phone: if demo {
                "912345678".into()
            } else {
                String::new()
            },
            nif: String::new(),
            confirm_write: false,
            last_probe: Instant::now() - Duration::from_secs(2),
            last_poll: Instant::now(),
            recovery: String::new(),
            recovering: false,
            demo,
            close_dialog: false,
            allow_close: false,
            show_other: false,
            balance_only: false,
            probe_failed: false,
        }
    }
    fn send(&mut self, c: Command) {
        if self.busy {
            return;
        }
        self.error.clear();
        self.busy = true;
        if self.tx.send(c).is_err() {
            self.busy = false;
            self.error = "O serviço da aplicação terminou. Reabre a aplicação.".into()
        }
    }
    fn start(&mut self) {
        self.step = Step::Detect;
        self.error.clear();
        self.probe_failed = false;
        self.last_probe = Instant::now() - Duration::from_secs(2);
    }
    fn active_purchase(&self) -> bool {
        let state = self.purchase["state"].as_str().unwrap_or("");
        !state.is_empty() && state != "loaded"
    }
    fn receive(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            self.busy = false;
            if !reply.purchase["state"].as_str().unwrap_or("").is_empty() {
                self.purchase = reply.purchase;
            }
            match reply.result {
                Err(e) => {
                    self.error = e;
                    self.probe_failed = reply.kind == "probe";
                    if self.active_purchase() {
                        self.step = Step::Payment;
                    }
                }
                Ok(v) => match reply.kind {
                    "probe" => {
                        self.probe_failed = false;
                        self.readers = model::rows(&v["readers"])
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect();
                        self.reader = model::text(&v["reader"]);
                        if v["present"] == true && self.step == Step::Detect {
                            if self.recovering {
                                self.send(Command::Recover {
                                    sale: self.recovery.trim().into(),
                                    reader: self.reader.clone(),
                                })
                            } else {
                                self.step = Step::Reading;
                                self.send(Command::Read(self.reader.clone()));
                            }
                        }
                    }
                    "read" => {
                        self.card = v;
                        self.step = Step::Summary;
                        self.products.clear();
                        self.details = Value::Null;
                        self.quote = Value::Null;
                        self.purchase = Value::Null;
                    }
                    "catalog" => {
                        self.products.clear();
                        model::products(&v, &mut self.products);
                        self.step = Step::Catalog;
                    }
                    "product" => {
                        self.details = v;
                        self.variant = model::rows(&self.details["prices"])
                            .iter()
                            .find(|p| p["blockedReason"].is_null())
                            .map(|p| model::text(&p["id"]))
                            .unwrap_or_default();
                        self.step = Step::Options;
                    }
                    "prepare" => {
                        self.quote = v;
                        self.step = Step::Checkout;
                    }
                    "checkout" | "recover" => {
                        self.purchase = v;
                        self.step = Step::Payment;
                        self.last_poll = Instant::now() - Duration::from_secs(5);
                    }
                    "payment" | "load" => {
                        self.purchase = v;
                        self.step = if self.purchase["state"] == "loaded" {
                            Step::Done
                        } else {
                            Step::Payment
                        };
                        self.last_poll = Instant::now();
                    }
                    _ => {}
                },
            }
        }
    }
    fn primary(ui: &mut egui::Ui, label: &str) -> bool {
        ui.add(
            egui::Button::new(RichText::new(label).color(Color32::BLACK).strong())
                .fill(YELLOW)
                .min_size(Vec2::new(220., 52.)),
        )
        .clicked()
    }
    fn title(ui: &mut egui::Ui, kicker: &str, title: &str, sub: &str) {
        ui.label(
            RichText::new(kicker)
                .color(YELLOW)
                .size(11.)
                .extra_letter_spacing(2.),
        );
        ui.add_space(12.);
        ui.heading(title);
        ui.add_space(4.);
        ui.label(RichText::new(sub).color(MUTED));
        ui.add_space(22.);
    }
    fn panel(ui: &mut egui::Ui, render: impl FnOnce(&mut egui::Ui)) {
        egui::Frame::new()
            .fill(Color32::from_rgb(21, 28, 29))
            .stroke(egui::Stroke::new(1_f32, Color32::from_rgb(44, 56, 57)))
            .corner_radius(16)
            .inner_margin(24)
            .show(ui, |ui| {
                ui.set_min_width((ui.available_width() - 2.).max(0.));
                render(ui);
            });
    }
    fn card(&self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(YELLOW)
            .corner_radius(20)
            .inner_margin(26)
            .show(ui, |ui| {
                ui.set_width((ui.available_width() - 2.).max(0.));
                ui.visuals_mut().override_text_color = Some(Color32::from_rgb(24, 28, 10));
                ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                    Color32::from_rgb(24, 28, 10);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("navegante®").size(27.).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Lisboa.");
                    });
                });
                ui.add_space(16.);
                ui.label(RichText::new(model::holder(&self.card)).size(23.).strong());
                ui.label(RichText::new(model::text(&self.card["media"]["cardNumber"])).size(13.));
                ui.add_space(14.);
                let mut titles = false;
                for item in model::rows(&self.card["contracts"]) {
                    if model::stored(item) {
                        let cents = model::field(&item["contract"], "contract.amount")
                            .parse::<i64>()
                            .ok();
                        ui.horizontal(|ui| {
                            ui.label("Saldo ocasionais");
                            ui.label(
                                RichText::new(
                                    cents
                                        .map(model::money)
                                        .unwrap_or_else(|| "Não disponível".into()),
                                )
                                .strong()
                                .size(21.),
                            );
                        });
                    } else {
                        titles = true;
                        ui.label(RichText::new(model::product_name(item)).strong());
                        ui.label(format!(
                            "{} · {}",
                            model::title_state(item, model::today()),
                            model::date(&model::field(
                                &item["contract"],
                                "contract.temporal_validity.date"
                            ))
                        ));
                    }
                }
                if !titles {
                    ui.label("Sem passe mensal indicado na leitura.");
                }
                ui.add_space(12.);
                let expiry = model::display(&self.card["issuing"], "issuing.expiration_date");
                if !expiry.is_empty() {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Cartão válido até {}", model::date(&expiry)))
                            .size(12.),
                    );
                }
            });
    }
    fn content(&mut self, ui: &mut egui::Ui) {
        match self.step {
            Step::Home => {
                Self::title(
                    ui,
                    "O TEU PASSE. NO TEU COMPUTADOR.",
                    "A próxima viagem começa aqui.",
                    "Consulta o cartão e carrega o teu passe, sem criar conta.",
                );
                Self::panel(ui, |ui| {
                    ui.label(RichText::new("Liga. Insere. Segue.").size(23.).strong());
                    ui.label("Só precisas do teu cartão Navegante e de um leitor USB PC/SC.");
                    ui.add_space(20.);
                    if Self::primary(ui, "Carregar passe") {
                        self.recovering = false;
                        self.start();
                    }
                });
                ui.add_space(22.);
                egui::CollapsingHeader::new("Já paguei e preciso de concluir um carregamento")
                    .show(ui, |ui| {
                        ui.label("Referência do pedido");
                        ui.text_edit_singleline(&mut self.recovery);
                        if ui
                            .add_enabled(
                                !self.recovery.trim().is_empty(),
                                egui::Button::new("Recuperar pedido"),
                            )
                            .clicked()
                        {
                            self.recovering = true;
                            self.start();
                        }
                    });
            }
            Step::Detect | Step::Reading => {
                let title = if self.step == Step::Reading {
                    "A ler o teu passe…"
                } else if self.readers.is_empty() {
                    "Liga o leitor USB."
                } else if self.reader.is_empty() {
                    "Escolhe o teu leitor."
                } else {
                    "Insere o cartão no leitor."
                };
                Self::title(
                    ui,
                    "PREPARAR O CARTÃO",
                    title,
                    "Avançamos automaticamente assim que estiver tudo pronto.",
                );
                Self::panel(ui, |ui| {
                    if self.step == Step::Reading {
                        ui.spinner();
                        ui.label(
                            "Mantém o cartão inserido. A leitura consulta o serviço Navegante.",
                        );
                    } else {
                        ui.label(if self.readers.is_empty() {
                            "À procura de um leitor compatível…"
                        } else if self.reader.is_empty() {
                            "Foi detetado mais de um leitor. Escolhe onde está o passe."
                        } else {
                            "Leitor ligado. À espera do cartão…"
                        });
                        if !self.readers.is_empty() {
                            egui::ComboBox::from_id_salt("reader")
                                .selected_text(if self.reader.is_empty() {
                                    "Selecionar leitor"
                                } else {
                                    &self.reader
                                })
                                .width(320.)
                                .show_ui(ui, |ui| {
                                    for reader in &self.readers {
                                        ui.selectable_value(
                                            &mut self.reader,
                                            reader.clone(),
                                            reader,
                                        );
                                    }
                                });
                        }
                        if self.busy {
                            ui.spinner();
                        }
                    }
                });
                if !self.error.is_empty() && Self::primary(ui, "Tentar novamente") {
                    self.start();
                }
                if ui.button("Voltar ao início").clicked() {
                    self.step = Step::Home;
                }
            }
            Step::Summary => {
                Self::title(
                    ui,
                    "CARTÃO LIDO",
                    "O que queres fazer?",
                    "Os dados abaixo correspondem à última leitura do cartão.",
                );
                self.card(ui);
                ui.add_space(22.);
                ui.horizontal_wrapped(|ui| {
                    if Self::primary(ui, "Carregar um passe") {
                        self.show_other = false;
                        self.balance_only = false;
                        self.send(Command::Catalog)
                    }
                    if ui.button("Adicionar saldo").clicked() {
                        self.show_other = true;
                        self.balance_only = true;
                        self.send(Command::Catalog)
                    }
                });
                egui::CollapsingHeader::new("Consultar perfis do cartão").show(ui, |ui| {
                    for p in model::rows(&self.card["profiles"]) {
                        let name = model::display(&p["profile"], "profile.name");
                        let expiry = model::display(&p["profile"], "profile.end_validity");
                        if !name.is_empty() {
                            ui.label(format!("{name}   {}", model::date(&expiry)));
                        }
                    }
                });
            }
            Step::Catalog => {
                Self::title(
                    ui,
                    "ESCOLHER CARREGAMENTO",
                    "Qual é a tua próxima viagem?",
                    "Produtos disponíveis para o teu cartão, devolvidos pelo serviço oficial.",
                );
                if self.products.is_empty() {
                    ui.label("Não foram devolvidos produtos. Volta a consultar mais tarde.");
                }
                let products = self.products.clone();
                let mut selected = None;
                let active_metro = model::rows(&self.card["contracts"]).iter().any(|i| {
                    model::product_name(i)
                        .to_lowercase()
                        .contains("metropolitano")
                        && model::title_state(i, model::today()) == "Válido"
                });
                for p in &products {
                    let name = model::text(&p["name"]);
                    let balance = name.to_lowercase().contains("zapping")
                        || name.to_lowercase().contains("pré-pago")
                        || p["id"] == "id-prod-zapping";
                    if self.balance_only && !balance {
                        continue;
                    }
                    let municipal = name.to_lowercase().contains("municipal")
                        || (!name.to_lowercase().contains("metropolitano")
                            && model::text(&p["id"]).starts_with("id-prod-navegante-"));
                    if active_metro && municipal && !self.show_other {
                        continue;
                    }
                    if ui
                        .add_sized([ui.available_width(), 54.], egui::Button::new(name))
                        .clicked()
                    {
                        selected = Some(model::text(&p["id"]));
                    }
                }
                if self.balance_only && ui.button("Ver também os passes disponíveis").clicked() {
                    self.balance_only = false;
                }
                if active_metro {
                    ui.checkbox(&mut self.show_other, "Mostrar também passes municipais");
                    ui.label(
                        RichText::new(
                            "Já tens Metropolitano válido. O mesmo período fica bloqueado.",
                        )
                        .color(MUTED)
                        .size(13.),
                    );
                }
                if let Some(pid) = selected {
                    self.send(Command::Product(pid));
                }
                if ui.button("Ver o meu cartão").clicked() {
                    self.step = Step::Summary;
                }
            }
            Step::Options => {
                Self::title(
                    ui,
                    "TÍTULO OU SALDO",
                    &model::text(&self.details["name"]),
                    "Escolhe uma das opções disponíveis para este cartão.",
                );
                let prices = model::rows(&self.details["prices"]).to_vec();
                let details = self.details.clone();
                Self::panel(ui, |ui| {
                    for price in &prices {
                        let amount = service::price_amount(price)
                            .map(model::money)
                            .unwrap_or_else(|_| "Preço indisponível".into());
                        let label =
                            format!("{}   {}", model::option_label(&details, price), amount);
                        let blocked = price["blockedReason"].as_str();
                        ui.add_enabled_ui(blocked.is_none(), |ui| {
                            ui.radio_value(&mut self.variant, model::text(&price["id"]), label);
                        });
                        if let Some(reason) = blocked {
                            ui.label(RichText::new(reason).size(12.).color(MUTED));
                        }
                    }
                    ui.add_space(12.);
                    if !self.variant.is_empty() && Self::primary(ui, "Rever carregamento") {
                        self.send(Command::Prepare(self.variant.clone()));
                    }
                });
                if ui.button("Escolher outro produto").clicked() {
                    self.step = Step::Catalog;
                }
            }
            Step::Checkout => {
                Self::title(
                    ui,
                    "CONFIRMAR",
                    "Tudo pronto para continuar.",
                    "O pagamento só é pedido quando confirmares abaixo.",
                );
                Self::panel(ui, |ui| {
                    ui.label(model::text(&self.card["media"]["cardNumber"]));
                    ui.label(
                        RichText::new(model::text(&self.quote["name"]))
                            .strong()
                            .size(21.),
                    );
                    if let Some(p) = model::rows(&self.details["prices"])
                        .iter()
                        .find(|p| p["id"] == self.quote["variant"])
                    {
                        ui.label(model::option_label(&self.details, p));
                    }
                    let amount = self.quote["amount"].as_i64().unwrap_or(0);
                    ui.label(
                        RichText::new(model::money(amount))
                            .color(YELLOW)
                            .size(34.)
                            .strong(),
                    );
                    ui.separator();
                    ui.label("E-mail para o recibo");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.email).desired_width(f32::INFINITY),
                    );
                    if amount > 0 {
                        ui.label("Número MB WAY");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.phone)
                                .hint_text("9 dígitos")
                                .char_limit(9)
                                .desired_width(f32::INFINITY),
                        );
                    }
                    ui.label("NIF (opcional)");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.nif)
                            .char_limit(9)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(8.);
                    let label = if self.demo {
                        format!("Simular pagamento de {}", model::money(amount))
                    } else if amount == 0 {
                        "Confirmar carregamento gratuito".into()
                    } else {
                        format!("Pagar {} com MB WAY", model::money(amount))
                    };
                    if Self::primary(ui, &label) {
                        self.send(Command::Checkout {
                            email: self.email.clone(),
                            phone: self.phone.clone(),
                            nif: self.nif.clone(),
                            amount,
                        });
                    }
                });
                if ui.button("Alterar opção").clicked() {
                    self.step = Step::Options;
                }
            }
            Step::Payment => {
                let state = self.purchase["state"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_owned();
                let (title, sub) = match state.as_str() {
                    "paid" => (
                        "Pagamento confirmado.",
                        "Falta gravar o carregamento no cartão. Mantém o passe inserido.",
                    ),
                    "uncertain" => (
                        "A confirmar a gravação.",
                        "Atualiza o estado. Não repitas o pagamento nem a gravação.",
                    ),
                    "canceled" => (
                        "O pagamento foi cancelado.",
                        "Consulta a referência se precisares de ajuda com o pedido.",
                    ),
                    "refunded" => (
                        "Pagamento reembolsado.",
                        "O serviço confirmou o reembolso deste pedido.",
                    ),
                    "refund_pending" => (
                        "Reembolso em processamento.",
                        "Consulta o estado mais tarde com esta referência.",
                    ),
                    "pending" => (
                        "Confirma no MB WAY.",
                        "Abre o MB WAY e aprova o pagamento. O estado é atualizado automaticamente.",
                    ),
                    _ => (
                        "Vamos confirmar o teu pedido.",
                        "Consulta o estado ou usa a referência junto do apoio Navegante antes de repetir.",
                    ),
                };
                Self::title(ui, "O TEU PEDIDO", title, sub);
                Self::panel(ui, |ui| {
                    let sale = model::text(&self.purchase["saleId"]);
                    ui.label("Referência do pedido — guarda-a para poderes recuperar:");
                    if sale.is_empty() {
                        ui.label("O serviço não devolveu referência. Contacta o apoio Navegante antes de repetir.");
                        ui.label("Se recebeste a referência por e-mail, podes recuperá-la aqui:");
                        ui.text_edit_singleline(&mut self.recovery);
                        if ui
                            .add_enabled(
                                !self.recovery.trim().is_empty(),
                                egui::Button::new("Recuperar pedido"),
                            )
                            .clicked()
                        {
                            self.send(Command::Recover {
                                sale: self.recovery.trim().into(),
                                reader: self.reader.clone(),
                            });
                        }
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(&sale).monospace());
                            if ui.small_button("Copiar referência").clicked() {
                                ui.ctx().copy_text(sale.clone())
                            }
                        });
                        if ui.button("Atualizar estado").clicked() {
                            self.send(Command::Payment);
                        }
                    }
                    if self.purchase["canLoad"] == true {
                        ui.separator();
                        ui.checkbox(
                            &mut self.confirm_write,
                            "O passe da compra está inserido. Confirmo a gravação.",
                        );
                        ui.add_enabled_ui(self.confirm_write, |ui| {
                            if Self::primary(
                                ui,
                                if self.demo {
                                    "Simular carregamento"
                                } else {
                                    "Gravar carregamento no passe"
                                },
                            ) {
                                self.send(Command::Load)
                            }
                        });
                    }
                });
                if matches!(state.as_str(), "canceled" | "refunded")
                    && ui.button("Voltar ao início").clicked()
                {
                    self.purchase = Value::Null;
                    self.step = Step::Home;
                }
            }
            Step::Done => {
                Self::title(
                    ui,
                    "CARREGAMENTO CONCLUÍDO",
                    "Boa viagem.",
                    if self.demo {
                        "Simulação concluída. Não houve pagamento ou alteração de um cartão real."
                    } else {
                        "O serviço oficial confirmou que o carregamento ficou gravado no cartão."
                    },
                );
                Self::panel(ui, |ui| {
                    ui.label(RichText::new("Podes retirar o passe.").size(24.).strong());
                    ui.label(format!("Pedido: {}", model::text(&self.purchase["saleId"])));
                    if Self::primary(ui, "Voltar ao início") {
                        self.step = Step::Home;
                        self.purchase = Value::Null;
                        self.card = Value::Null;
                        self.confirm_write = false;
                        self.error.clear();
                    }
                });
            }
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive();
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.error = "Aguarda que a operação termine antes de fechar a aplicação.".into();
            } else if self.active_purchase() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_dialog = true;
            }
        }
        if self.step == Step::Detect
            && !self.busy
            && (self.error.is_empty() || self.probe_failed)
            && self.last_probe.elapsed() > Duration::from_millis(900)
        {
            self.last_probe = Instant::now();
            self.send(Command::Probe(self.reader.clone()));
        }
        if self.step == Step::Payment
            && !self.busy
            && self.error.is_empty()
            && self.purchase["state"] == "pending"
            && self.last_poll.elapsed() > Duration::from_secs(4)
        {
            self.last_poll = Instant::now();
            self.send(Command::Payment);
        }
        egui::TopBottomPanel::top("brand")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(28, 20)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (logo, _) = ui.allocate_exact_size(Vec2::splat(34.), egui::Sense::hover());
                    ui.painter().rect_filled(logo, 8., YELLOW);
                    let point = |x: f32, y: f32| logo.min + Vec2::new(x, y);
                    ui.painter().add(egui::Shape::line(
                        vec![
                            point(10., 25.),
                            point(10., 10.),
                            point(14., 10.),
                            point(24., 25.),
                            point(24., 10.),
                            point(18., 10.),
                        ],
                        egui::Stroke::new(2.4_f32, Color32::BLACK),
                    ));
                    ui.label(RichText::new("navegante").size(22.).strong());
                    ui.label(RichText::new("DESKTOP").size(10.).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.demo {
                            ui.label(RichText::new("DEMONSTRAÇÃO · SEM COBRANÇAS").color(YELLOW));
                        } else {
                            ui.label(RichText::new("No teu computador.").color(MUTED).size(12.));
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(28, 12)))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Projeto independente · Sem afiliação à TML")
                            .color(MUTED)
                            .size(11.),
                    );
                    if !self.busy
                        && !self.active_purchase()
                        && self.step != Step::Home
                        && ui.small_button("Início").clicked()
                    {
                        self.step = Step::Home;
                        self.error.clear();
                    }
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let width = ui.available_width().min(720.);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - width) / 2.).max(0.));
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add_space(28.);
                        ui.add_enabled_ui(!self.busy, |ui| self.content(ui));
                        if self.busy {
                            ui.add_space(14.);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(if self.step == Step::Payment {
                                    "A consultar o pedido… Mantém o cartão inserido."
                                } else {
                                    "A processar…"
                                });
                            });
                        }
                        if !self.error.is_empty() {
                            ui.add_space(14.);
                            ui.label(
                                RichText::new(&self.error).color(Color32::from_rgb(255, 165, 135)),
                            );
                            if self.step == Step::Reading
                                && ui.button("Tentar ler novamente").clicked()
                            {
                                self.start();
                            }
                            if self.step == Step::Summary
                                && ui.button("Voltar a consultar produtos").clicked()
                            {
                                self.send(Command::Catalog);
                            }
                        }
                        ui.add_space(24.);
                    });
                });
            });
        });
        if self.close_dialog {
            egui::Window::new("Sair com um pedido em curso?")
                .collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO)
                .show(ctx,|ui| {
                    ui.label("Guarda a referência para recuperar este pedido. O pagamento não é cancelado ao fechar.");
                    ui.label(model::text(&self.purchase["saleId"]));
                    if ui.button("Continuar na aplicação").clicked() { self.close_dialog=false; }
                    if ui.button("Guardei a referência. Fechar").clicked() {
                        self.allow_close=true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
        }
        if self.step == Step::Detect
            || (self.step == Step::Payment && self.purchase["state"] == "pending")
            || self.busy
        {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}
#[derive(Default)]
struct Demo {
    purchase: Value,
    details: Value,
    amount: i64,
    card: Value,
    wait: bool,
    started: Option<Instant>,
}
impl Demo {
    fn run(&mut self, c: Command) -> Result<Value, String> {
        let result = match c {
            Command::Probe(_) => {
                let elapsed = self.started.get_or_insert_with(Instant::now).elapsed();
                if self.wait && elapsed < Duration::from_secs(2) {
                    json!({"readers":[],"reader":"","present":false})
                } else {
                    json!({"readers":["Leitor de demonstração"],"reader":"Leitor de demonstração","present":!self.wait || elapsed>Duration::from_secs(4)})
                }
            }
            Command::Read(_) => {
                if self.card.is_null() {
                    self.card = json!({"media":{"cardNumber":"DEMO 0000 0000"},"holder":[{"id":"holder.name","value":"Alex · Cartão demo"}],"issuing":[{"id":"issuing.expiration_date","value":"2031-06-30"}],"contracts":[{"product_detail":{"product":{"name":"Zapping","id":"id-prod-zapping"}},"contract":[{"id":"contract.amount","value_id":"1235"}]}]});
                }
                self.card.clone()
            }
            Command::Catalog => {
                json!({"products":[{"id":"id-prod-navegante-metro","name":"navegante Metropolitano"},{"id":"id-prod-zapping","name":"Saldo Zapping"}]})
            }
            Command::Product(pid) => {
                use chrono::Datelike;
                self.amount = if pid == "id-prod-zapping" { 1000 } else { 4000 };
                self.details = json!({"id":pid,"name":if pid=="id-prod-zapping"{"Saldo Zapping"}else{"navegante Metropolitano"},"attributes":[{"id":if pid=="id-prod-zapping"{"product_occasional"}else{"product_monthly"},"values":[{"id":"demo-option","value":if pid=="id-prod-zapping"{"1000".into()}else{model::today().month().to_string()}}]}],"prices":[{"id":"demo-option","description":"Exemplo fictício","net":self.amount,"tax":0}]});
                self.details["prices"][0]["blockedReason"] = json!(model::blocked(
                    &self.card,
                    &pid,
                    &self.details,
                    &json!("demo-option"),
                    model::today()
                ));
                self.details.clone()
            }
            Command::Prepare(_) => {
                json!({"name":self.details["name"],"variant":"demo-option","amount":self.amount})
            }
            Command::Checkout { .. } | Command::Recover { .. } => {
                self.purchase = json!({"saleId":"DEMO-SEM-COBRANCA","state":"pending"});
                self.purchase.clone()
            }
            Command::Payment => {
                self.purchase = json!({"saleId":"DEMO-SEM-COBRANCA","state":"paid","canLoad":true});
                self.purchase.clone()
            }
            Command::Load => {
                if self.details["id"] == "id-prod-navegante-metro" {
                    use chrono::Datelike;
                    let now = model::today();
                    let expiry = now
                        .with_day(1)
                        .unwrap()
                        .checked_add_months(chrono::Months::new(1))
                        .unwrap()
                        .pred_opt()
                        .unwrap();
                    if let Some(contracts) = self.card["contracts"].as_array_mut() {
                        contracts.push(json!({"product_detail":{"product":{"id":"id-prod-navegante-metro","name":"navegante Metropolitano"}},"contract":[{"id":"contract.temporal_validity.date","value_id":expiry.to_string()},{"id":"contract.temporal_validity.status","value_id":"1"}]}));
                    }
                }
                if self.details["id"] == "id-prod-zapping"
                    && let Some(fields) = self.card["contracts"][0]["contract"].as_array_mut()
                    && let Some(f) = fields.iter_mut().find(|f| f["id"] == "contract.amount")
                {
                    let balance = model::text(&f["value_id"]).parse::<i64>().unwrap_or(0);
                    f["value_id"] = json!((balance + self.amount).to_string());
                }
                self.purchase = json!({"saleId":"DEMO-SEM-COBRANCA","state":"loaded"});
                self.purchase.clone()
            }
        };
        Ok(result)
    }
}
fn main() -> eframe::Result {
    if let Some(reader) = std::env::args().skip_while(|a| a != "--read-check").nth(1) {
        let result = (|| -> anyhow::Result<()> {
            let ctx = pcsc::Context::new()?;
            let mut card = ctx.connect(&reader)?;
            let (_, data) = protocol::read(&api::Api::new()?, |c| card.transmit(c))?;
            println!(
                "Official Rust read OK; {} contracts; no payment or write requested.",
                model::rows(&data["contracts"]).len()
            );
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--self-test") {
        let result = (|| -> anyhow::Result<()> {
            let context = pcsc::Context::new()?;
            let readers = context.readers()?;
            println!(
                "Rust native runtime OK; {} PC/SC reader(s); no browser or server.",
                readers.len()
            );
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1)
        }
        return Ok(());
    }
    let demo = std::env::args().any(|a| a == "--demo" || a == "--demo-wait");
    let image = image::load_from_memory(include_bytes!("../../desktop-assets/app.ico"))
        .expect("Bundled icon")
        .into_rgba8();
    let icon = egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000., 800.])
            .with_min_inner_size([680., 580.])
            .with_icon(icon),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        if demo {
            "Navegante Desktop — Demo"
        } else {
            "Navegante Desktop"
        },
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, demo)))),
    )
}
