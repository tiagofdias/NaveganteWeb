#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod api;
mod design;
mod model;
mod pcsc;
mod protocol;
mod service;
#[cfg(test)]
mod test_support;
use design::{MUTED, YELLOW};
use eframe::egui::{self, Color32, RichText, Vec2};
use serde_json::{Value, json};
use service::Command;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
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
    epoch: u64,
    kind: &'static str,
    result: Result<Value, String>,
    purchase: Value,
}
struct App {
    tx: Sender<(u64, Command)>,
    rx: Receiver<Reply>,
    step: Step,
    busy: bool,
    probe_pending: bool,
    detection_epoch: u64,
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
        design::install(&cc.egui_ctx);
        let (tx, commands) = mpsc::channel();
        let (replies, rx) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let mut backend = service::Service::new().map_err(|e| e.to_string());
            let mut demo_backend = Demo {
                wait: std::env::args().any(|a| a == "--demo-wait"),
                ..Default::default()
            };
            while let Ok((epoch, command)) = commands.recv() {
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
                        epoch,
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
        Self::with_channels(tx, rx, demo)
    }
    fn with_channels(tx: Sender<(u64, Command)>, rx: Receiver<Reply>, demo: bool) -> Self {
        Self {
            tx,
            rx,
            step: Step::Home,
            busy: false,
            probe_pending: false,
            detection_epoch: 0,
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
        let probe = matches!(c, Command::Probe(_));
        if probe {
            if self.probe_pending || self.step != Step::Detect {
                return;
            }
            self.probe_pending = true;
        } else {
            self.error.clear();
            self.busy = true;
        }
        if self.tx.send((self.detection_epoch, c)).is_err() {
            self.busy = false;
            self.probe_pending = false;
            self.error = "O serviço da aplicação terminou. Reabre a aplicação.".into()
        }
    }
    fn start(&mut self) {
        self.detection_epoch = self.detection_epoch.wrapping_add(1);
        self.step = Step::Detect;
        self.error.clear();
        self.probe_failed = false;
        self.last_probe = Instant::now() - Duration::from_secs(2);
    }
    fn active_purchase(&self) -> bool {
        let state = self.purchase["state"].as_str().unwrap_or("");
        !state.is_empty() && state != "loaded"
    }
    fn go_home(&mut self) {
        if self.busy {
            return;
        }
        self.detection_epoch = self.detection_epoch.wrapping_add(1);
        self.step = Step::Home;
        self.error.clear();
    }
    fn should_probe(&self) -> bool {
        self.step == Step::Detect
            && !self.busy
            && !self.probe_pending
            && (self.readers.is_empty() || !self.reader.is_empty())
            && (self.error.is_empty() || self.probe_failed)
            && self.last_probe.elapsed() > Duration::from_millis(900)
    }
    fn receive(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            if reply.kind == "probe" {
                self.probe_pending = false;
                if self.step != Step::Detect || reply.epoch != self.detection_epoch {
                    continue;
                }
            } else {
                self.busy = false;
            }
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
                        if self.probe_failed {
                            self.error.clear();
                        }
                        self.probe_failed = false;
                        self.readers = model::rows(&v["readers"])
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect();
                        let detected_reader = model::text(&v["reader"]);
                        if self.reader.is_empty() {
                            self.reader = detected_reader.clone();
                        } else if !self.readers.contains(&self.reader) {
                            self.reader.clear();
                        }
                        if !self.busy
                            && !self.reader.is_empty()
                            && detected_reader == self.reader
                            && v["present"] == true
                        {
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
        design::primary(ui, label)
    }
    fn title(ui: &mut egui::Ui, kicker: &str, title: &str, sub: &str) {
        design::title(ui, kicker, title, sub);
    }
    fn panel(ui: &mut egui::Ui, render: impl FnOnce(&mut egui::Ui)) {
        design::panel(ui, render);
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        if self.active_purchase() {
            Self::title(
                ui,
                "O TEU PEDIDO",
                "Tens um carregamento por concluir.",
                "Retoma o pedido existente para acompanhar o pagamento e concluir a gravação.",
            );
            Self::panel(ui, |ui| {
                ui.label(format!(
                    "Referência: {}",
                    model::text(&self.purchase["saleId"])
                ));
                if Self::primary(ui, "Retomar pedido") {
                    self.step = Step::Payment;
                }
            });
            return;
        }
        let wide = ui.available_width() > 830.;
        ui.add_space(if wide { 38. } else { 6. });
        let mut hero = |ui: &mut egui::Ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            design::eyebrow(ui, "MOBILIDADE, SEM COMPLICAÇÕES");
            ui.add_space(24.);
            let size = if wide { 49. } else { 42. };
            ui.label(
                RichText::new("A tua próxima\nviagem.")
                    .font(design::display(size))
                    .line_height(Some(size * 1.17)),
            );
            ui.label(
                RichText::new("Começa aqui.")
                    .font(design::display(size))
                    .color(YELLOW),
            );
            ui.add_space(13.);
            ui.label(RichText::new("O teu passe, o teu saldo e toda a cidade.\nCarrega o Navegante sem sair de casa.").size(15.).color(MUTED).line_height(Some(24.)));
            ui.add_space(24.);
            if Self::primary(ui, "Carregar passe") {
                self.recovering = false;
                self.start();
            }
            ui.add_space(5.);
            ui.label(
                RichText::new("Sem criar conta. Ao teu ritmo.")
                    .size(11.)
                    .color(MUTED),
            );
        };
        if wide {
            let left = ui.available_width() * 0.54;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(left, 430.),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_width(left);
                        hero(ui);
                    },
                );
                ui.vertical(|ui| {
                    ui.add_space(20.);
                    design::pass_art(ui);
                });
            });
        } else {
            hero(ui);
        }
        ui.add_space(30.);
        ui.separator();
        ui.add_space(17.);
        ui.columns(3, |cols| {
            design::feature(
                &mut cols[0],
                "01",
                "Liga o leitor",
                "O teu leitor USB habitual.",
            );
            design::feature(
                &mut cols[1],
                "02",
                "Insere o cartão",
                "Tudo aparece automaticamente.",
            );
            design::feature(
                &mut cols[2],
                "03",
                "Carrega e segue",
                "Passe ou saldo. Tu escolhes.",
            );
        });
        ui.add_space(28.);
        egui::CollapsingHeader::new(
            RichText::new("Já paguei e preciso de concluir um carregamento")
                .size(12.)
                .color(MUTED),
        )
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
    fn card(&self, ui: &mut egui::Ui) {
        let compact = ui.ctx().content_rect().width() < 980.;
        egui::Frame::new()
            .fill(YELLOW)
            .corner_radius(25)
            .inner_margin(if compact { 20 } else { 30 })
            .show(ui, |ui| {
                ui.set_width((ui.available_width() - 2.).max(0.));
                ui.spacing_mut().item_spacing.y = if compact { 5. } else { 9. };
                ui.spacing_mut().interact_size.y = 24.;
                ui.visuals_mut().override_text_color = Some(Color32::from_rgb(24, 28, 10));
                ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                    Color32::from_rgb(24, 28, 10);
                ui.visuals_mut().widgets.active.fg_stroke.color = Color32::from_rgb(24, 28, 10);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("navegante®")
                            .font(design::display(if compact { 22. } else { 25. }))
                            .color(design::INK),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Lisboa.");
                    });
                });
                ui.add_space(if compact { 6. } else { 16. });
                ui.label(
                    RichText::new(model::holder(&self.card))
                        .font(design::display(if compact { 22. } else { 25. }))
                        .color(design::INK),
                );
                ui.label(RichText::new(model::text(&self.card["media"]["cardNumber"])).size(13.));
                ui.add_space(if compact { 6. } else { 14. });
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
                                .font(design::display(if compact { 28. } else { 32. }))
                                .color(design::INK),
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
                ui.add_space(if compact { 6. } else { 12. });
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
            Step::Home => self.home(ui),
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
                    design::device(ui, self.step == Step::Reading);
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
                            let previous_reader = self.reader.clone();
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
                            if self.reader != previous_reader {
                                self.last_probe = Instant::now() - Duration::from_secs(2);
                            }
                            if ui.small_button("Atualizar lista").clicked() {
                                self.send(Command::Probe(self.reader.clone()));
                            }
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
                        .add_sized(
                            [ui.available_width(), 72.],
                            egui::Button::new(RichText::new(name).font(design::strong(16.)))
                                .corner_radius(16),
                        )
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
                        let label = service::price_amount(price)
                            .map(|amount| model::choice_label(&details, price, amount))
                            .unwrap_or_else(|_| "Preço indisponível".into());
                        let blocked = price["blockedReason"].as_str();
                        let selected = self.variant == model::text(&price["id"]);
                        egui::Frame::new()
                            .fill(if selected {
                                Color32::from_rgb(33, 35, 22)
                            } else {
                                design::BG
                            })
                            .stroke(egui::Stroke::new(
                                1_f32,
                                if selected {
                                    Color32::from_rgb(118, 112, 32)
                                } else {
                                    design::LINE
                                },
                            ))
                            .corner_radius(14)
                            .inner_margin(16)
                            .show(ui, |ui| {
                                ui.set_min_width((ui.available_width() - 2.).max(0.));
                                ui.add_enabled_ui(blocked.is_none(), |ui| {
                                    ui.radio_value(
                                        &mut self.variant,
                                        model::text(&price["id"]),
                                        label,
                                    );
                                });
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
                let amount = self.quote["amount"].as_i64().unwrap_or(0);
                Self::panel(ui, |ui| {
                    ui.columns(2, |cols| {
                        let left = &mut cols[0];
                        left.label(
                            RichText::new("O TEU CARREGAMENTO")
                                .font(design::strong(10.))
                                .color(MUTED)
                                .extra_letter_spacing(1.4),
                        );
                        left.add_space(15.);
                        left.label(
                            RichText::new(model::text(&self.quote["name"]))
                                .font(design::display(20.)),
                        );
                        if let Some(p) = model::rows(&self.details["prices"])
                            .iter()
                            .find(|p| p["id"] == self.quote["variant"])
                        {
                            left.label(
                                RichText::new(model::option_label(&self.details, p)).color(MUTED),
                            );
                        }
                        left.add_space(12.);
                        left.label(
                            RichText::new(model::money(amount))
                                .font(design::display(40.))
                                .color(YELLOW),
                        );
                        left.add_space(24.);
                        left.separator();
                        left.label(
                            RichText::new("CARTÃO")
                                .font(design::strong(9.))
                                .color(MUTED)
                                .extra_letter_spacing(1.4),
                        );
                        left.label(
                            RichText::new(model::text(&self.card["media"]["cardNumber"])).size(12.),
                        );
                        let right = &mut cols[1];
                        right.spacing_mut().item_spacing.y = 10.;
                        right
                            .label(RichText::new("E-mail para o recibo").font(design::strong(12.)));
                        right.add(
                            egui::TextEdit::singleline(&mut self.email)
                                .margin(egui::Margin::symmetric(12, 11))
                                .desired_width(f32::INFINITY),
                        );
                        if amount > 0 {
                            right.label(RichText::new("Número MB WAY").font(design::strong(12.)));
                            right.add(
                                egui::TextEdit::singleline(&mut self.phone)
                                    .margin(egui::Margin::symmetric(12, 11))
                                    .hint_text("9 dígitos")
                                    .char_limit(9)
                                    .desired_width(f32::INFINITY),
                            );
                        }
                        right.label(RichText::new("NIF (opcional)").font(design::strong(12.)));
                        right.add(
                            egui::TextEdit::singleline(&mut self.nif)
                                .margin(egui::Margin::symmetric(12, 11))
                                .char_limit(9)
                                .desired_width(f32::INFINITY),
                        );
                        right.add_space(12.);
                        let label = if self.demo {
                            format!("Simular pagamento de {}", model::money(amount))
                        } else if amount == 0 {
                            "Confirmar carregamento gratuito".into()
                        } else {
                            format!("Pagar {} com MB WAY", model::money(amount))
                        };
                        if Self::primary(right, &label) {
                            self.send(Command::Checkout {
                                email: self.email.clone(),
                                phone: self.phone.clone(),
                                nif: self.nif.clone(),
                                amount,
                            });
                        }
                    });
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
                    ui.label(RichText::new("Podes retirar o passe.").font(design::display(24.)));
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
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(true));
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        } else if fullscreen && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.error = "Aguarda que a operação termine antes de fechar a aplicação.".into();
            } else if !self.allow_close && self.active_purchase() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_dialog = true;
            }
        }
        if self.should_probe() {
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
            .frame(
                egui::Frame::new()
                    .fill(design::BG)
                    .inner_margin(egui::Margin::symmetric(32, 18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    design::logo(ui);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.;
                        ui.label(RichText::new("navegante").font(design::display(19.)));
                        ui.label(
                            RichText::new("DESKTOP")
                                .font(design::strong(8.))
                                .extra_letter_spacing(3.)
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.;
                        if ui
                            .add_enabled_ui(!self.busy, |ui| {
                                design::header_control(ui, "Fechar", design::HeaderControl::Close)
                            })
                            .inner
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui
                            .scope(|ui| {
                                design::header_control(
                                    ui,
                                    if fullscreen {
                                        "Janela"
                                    } else {
                                        "Ecrã inteiro"
                                    },
                                    design::HeaderControl::Window(fullscreen),
                                )
                            })
                            .inner
                            .on_hover_text("F11 alterna o ecrã inteiro. Esc regressa à janela.")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                        }
                        ui.add_space(8.);
                        let (divider, _) =
                            ui.allocate_exact_size(Vec2::new(1., 18.), egui::Sense::hover());
                        ui.painter().line_segment(
                            [divider.center_top(), divider.center_bottom()],
                            egui::Stroke::new(1_f32, design::LINE),
                        );
                        ui.add_space(8.);
                        if ui
                            .add_enabled_ui(!self.busy && self.step != Step::Home, |ui| {
                                design::header_control(
                                    ui,
                                    "Voltar ao início",
                                    design::HeaderControl::Home,
                                )
                            })
                            .inner
                            .clicked()
                        {
                            self.go_home();
                        }
                        if ctx.content_rect().width() > 1100. {
                            design::pill(
                                ui,
                                if self.demo {
                                    "DEMONSTRAÇÃO · SEM COBRANÇAS"
                                } else if !self.card.is_null() {
                                    "CARTÃO LIDO"
                                } else {
                                    "PRONTO PARA COMEÇAR"
                                },
                                self.demo || !self.card.is_null(),
                            );
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
                });
            });
        let stage = match self.step {
            Step::Home | Step::Detect | Step::Reading => 0,
            Step::Summary | Step::Catalog | Step::Options => 1,
            Step::Checkout => 2,
            Step::Payment if self.purchase["canLoad"] != true => 2,
            _ => 3,
        };
        if self.step != Step::Home && ctx.content_rect().width() >= 980. {
            egui::SidePanel::left("journey")
                .exact_width(190.)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(design::BG)
                        .inner_margin(egui::Margin::symmetric(15, 18)),
                )
                .show(ctx, |ui| design::rail(ui, stage));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(design::BG)
                    .inner_margin(egui::Margin::symmetric(30, 12)),
            )
            .show(ctx, |ui| {
                design::ambient(ui.painter(), ui.max_rect());
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let width = ui.available_width().min(if self.step == Step::Home {
                        1050.
                    } else {
                        740.
                    });
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
                                    RichText::new(&self.error)
                                        .color(Color32::from_rgb(255, 165, 135)),
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
                    if ui.add_enabled(!self.busy,egui::Button::new("Guardei a referência. Fechar")).clicked() {
                        self.allow_close=true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
        }
        if (self.step == Step::Detect
            && (self.readers.is_empty() || !self.reader.is_empty() || self.probe_pending))
            || (self.step == Step::Payment && self.purchase["state"] == "pending")
            || self.busy
        {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}
#[cfg(test)]
mod detection_tests {
    use super::*;
    fn app() -> (App, Receiver<(u64, Command)>, Sender<Reply>) {
        let (tx, commands) = mpsc::channel();
        let (replies, rx) = mpsc::channel();
        let mut app = App::with_channels(tx, rx, false);
        app.start();
        (app, commands, replies)
    }
    fn reply(app: &mut App, replies: &Sender<Reply>, reader: &str, present: bool, epoch: u64) {
        replies
            .send(Reply {
                epoch,
                kind: "probe",
                result: Ok(json!({"readers":["A","B"],"reader":reader,"present":present})),
                purchase: Value::Null,
            })
            .unwrap();
        app.receive();
    }
    #[test]
    fn scanning_is_silent_and_waits_for_choice() {
        let (mut app, commands, replies) = app();
        app.send(Command::Probe(String::new()));
        assert!(!app.busy);
        assert!(app.probe_pending);
        let epoch = app.detection_epoch;
        reply(&mut app, &replies, "", false, epoch);
        assert!(matches!(commands.try_recv().unwrap().1, Command::Probe(_)));
        assert!(commands.try_recv().is_err());
        assert!(!app.should_probe());
        assert_eq!(app.step, Step::Detect);
        app.reader = "B".into();
        assert!(app.should_probe());
        app.send(Command::Probe("B".into()));
        commands.try_recv().unwrap();
        reply(&mut app, &replies, "B", true, epoch);
        assert!(matches!(commands.try_recv().unwrap().1,Command::Read(r) if r=="B"));
        assert!(app.busy);
    }
    #[test]
    fn old_probe_does_not_overwrite_a_new_selection() {
        let (mut app, commands, replies) = app();
        app.reader = "A".into();
        app.send(Command::Probe("A".into()));
        commands.try_recv().unwrap();
        app.reader = "B".into();
        let epoch = app.detection_epoch;
        reply(&mut app, &replies, "A", true, epoch);
        assert_eq!(app.reader, "B");
        assert_eq!(app.step, Step::Detect);
        assert!(commands.try_recv().is_err());
    }
    #[test]
    fn leaving_detection_ignores_late_probe() {
        let (mut app, commands, replies) = app();
        app.send(Command::Probe(String::new()));
        commands.try_recv().unwrap();
        app.step = Step::Home;
        let epoch = app.detection_epoch;
        reply(&mut app, &replies, "A", true, epoch);
        assert_eq!(app.step, Step::Home);
        assert!(commands.try_recv().is_err());
        assert!(!app.probe_pending);
    }
    #[test]
    fn restarting_detection_ignores_previous_generation() {
        let (mut app, commands, replies) = app();
        app.send(Command::Probe(String::new()));
        commands.try_recv().unwrap();
        let epoch = app.detection_epoch;
        app.start();
        reply(&mut app, &replies, "A", true, epoch);
        assert!(app.reader.is_empty());
        assert!(commands.try_recv().is_err());
        assert!(app.should_probe());
    }
    #[test]
    fn probe_does_not_unlock_foreground_operation() {
        let (mut app, commands, replies) = app();
        app.send(Command::Probe(String::new()));
        commands.try_recv().unwrap();
        app.send(Command::Catalog);
        commands.try_recv().unwrap();
        let epoch = app.detection_epoch;
        reply(&mut app, &replies, "A", true, epoch);
        assert!(app.busy);
        assert_eq!(app.step, Step::Detect);
        assert!(commands.try_recv().is_err());
    }
    #[test]
    fn background_retry_keeps_error_stable_until_success() {
        let (mut app, _commands, replies) = app();
        app.probe_failed = true;
        app.error = "Reader unavailable".into();
        app.send(Command::Probe(String::new()));
        assert_eq!(app.error, "Reader unavailable");
        let epoch = app.detection_epoch;
        reply(&mut app, &replies, "", false, epoch);
        assert!(app.error.is_empty());
        assert!(!app.probe_failed);
    }
    #[test]
    fn home_keeps_unfinished_purchase_for_resume() {
        let (mut app, _commands, _replies) = app();
        app.step = Step::Payment;
        app.purchase = json!({"saleId":"test-sale","state":"paid","canLoad":true});
        app.go_home();
        assert_eq!(app.step, Step::Home);
        assert_eq!(app.purchase["saleId"], "test-sale");
        assert!(app.active_purchase());
        assert_eq!(app.purchase["canLoad"], true);
    }
    #[test]
    fn home_cannot_interrupt_card_operations() {
        let (mut app, _commands, _replies) = app();
        app.step = Step::Reading;
        app.busy = true;
        app.go_home();
        assert_eq!(app.step, Step::Reading);
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
    if let Some(reader) = std::env::args()
        .skip_while(|a| a != "--review-check")
        .nth(1)
    {
        let result = (|| -> anyhow::Result<()> {
            let mut service = service::Service::new()?;
            service.run(Command::Read(reader))?;
            service.run(Command::Catalog)?;
            let details = service.run(Command::Product("id-prod-zapping".into()))?;
            let price = model::rows(&details["prices"])
                .iter()
                .find(|p| service::price_amount(p).ok() == Some(1500))
                .ok_or_else(|| anyhow::anyhow!("Opção de 15 euros não disponível."))?;
            let quote = service.run(Command::Prepare(model::text(&price["id"])))?;
            println!(
                "Official Rust price review OK; {} cents. No payment or card write requested.",
                quote["amount"]
            );
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
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
            .with_fullscreen(true)
            .with_min_inner_size([760., 640.])
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
