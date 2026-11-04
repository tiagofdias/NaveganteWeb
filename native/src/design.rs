//! Native, vector-drawn interface. No webview or image rendering dependency.
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Stroke, Vec2};
pub const YELLOW: Color32 = Color32::from_rgb(255, 224, 0);
pub const INK: Color32 = Color32::from_rgb(19, 25, 20);
pub const MUTED: Color32 = Color32::from_rgb(137, 153, 164);
pub const BG: Color32 = Color32::from_rgb(10, 14, 19);
pub const SURFACE: Color32 = Color32::from_rgb(18, 24, 32);
pub const LINE: Color32 = Color32::from_rgb(40, 50, 62);
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}
pub fn strong(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("strong".into()))
}
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, data) in [
        (
            "inter",
            include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice(),
        ),
        (
            "strong",
            include_bytes!("../assets/fonts/Inter-Semibold.ttf").as_slice(),
        ),
        (
            "display",
            include_bytes!("../assets/fonts/Sora-Display.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(data).into());
    }
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "inter".into());
    for name in ["strong", "display"] {
        let mut fallback = fonts.families[&FontFamily::Proportional].clone();
        fallback.insert(0, name.into());
        fonts
            .families
            .insert(FontFamily::Name(name.into()), fallback);
    }
    ctx.set_fonts(fonts);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.override_text_color = Some(Color32::from_rgb(239, 244, 247));
    v.extreme_bg_color = Color32::from_rgb(12, 18, 25);
    v.faint_bg_color = SURFACE;
    v.window_corner_radius = 20.into();
    v.window_stroke = Stroke::new(1_f32, LINE);
    v.selection.bg_fill = YELLOW.gamma_multiply(0.2);
    v.selection.stroke = Stroke::new(1_f32, YELLOW);
    v.hyperlink_color = YELLOW;
    for widget in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        widget.corner_radius = 12.into();
        widget.bg_stroke = Stroke::new(1_f32, LINE);
    }
    v.widgets.inactive.bg_fill = Color32::from_rgb(22, 30, 40);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(22, 30, 40);
    v.widgets.hovered.bg_fill = Color32::from_rgb(33, 43, 55);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(33, 43, 55);
    v.widgets.hovered.bg_stroke = Stroke::new(1_f32, Color32::from_rgb(79, 94, 108));
    v.widgets.active.bg_fill = Color32::from_rgb(42, 51, 61);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1_f32, LINE);
    ctx.set_visuals(v);
    let mut s = (*ctx.style()).clone();
    s.spacing.item_spacing = Vec2::new(14., 14.);
    s.spacing.button_padding = Vec2::new(22., 14.);
    s.spacing.interact_size.y = 38.;
    s.spacing.text_edit_width = 300.;
    s.spacing.window_margin = 24.into();
    s.animation_time = 0.15;
    s.text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(15.));
    s.text_styles.insert(egui::TextStyle::Button, strong(14.));
    s.text_styles.insert(egui::TextStyle::Heading, display(32.));
    s.text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.));
    ctx.set_style(s);
}
pub fn primary(ui: &mut egui::Ui, label: &str) -> bool {
    let response = ui.add(
        egui::Button::new(RichText::new(label).font(strong(15.)).color(INK))
            .fill(YELLOW)
            .stroke(Stroke::NONE)
            .corner_radius(14)
            .min_size(Vec2::new(224., 55.)),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}
#[derive(Clone, Copy)]
pub enum HeaderControl {
    Home,
    Window(bool),
    Close,
}
pub fn header_control(ui: &mut egui::Ui, label: &str, control: HeaderControl) -> egui::Response {
    let width = if matches!(control, HeaderControl::Home) {
        152.
    } else {
        38.
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 38.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let enabled = ui.is_enabled();
    let hover = enabled && response.hovered();
    let close_hover = hover && matches!(control, HeaderControl::Close);
    let color = if !enabled {
        Color32::from_rgb(84, 98, 110)
    } else if close_hover {
        Color32::from_rgb(255, 176, 171)
    } else if hover {
        Color32::WHITE
    } else {
        Color32::from_rgb(178, 192, 203)
    };
    if hover || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            10.,
            if close_hover {
                Color32::from_rgb(63, 30, 35)
            } else {
                Color32::from_rgb(25, 34, 44)
            },
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            10.,
            Stroke::new(1_f32, YELLOW),
            egui::StrokeKind::Inside,
        );
    }
    let p = ui.painter();
    let center = rect.center();
    let stroke = Stroke::new(1.6_f32, color);
    match control {
        HeaderControl::Home => {
            let c = Pos2::new(rect.min.x + 17., center.y);
            p.line_segment([c + Vec2::new(6., 0.), c + Vec2::new(-6., 0.)], stroke);
            p.add(egui::Shape::line(
                vec![
                    c + Vec2::new(-1., -5.),
                    c + Vec2::new(-6., 0.),
                    c + Vec2::new(-1., 5.),
                ],
                stroke,
            ));
            p.text(
                Pos2::new(rect.min.x + 35., center.y),
                Align2::LEFT_CENTER,
                label,
                strong(12.),
                color,
            );
        }
        HeaderControl::Close => {
            p.line_segment([center - Vec2::splat(5.), center + Vec2::splat(5.)], stroke);
            p.line_segment(
                [center + Vec2::new(-5., 5.), center + Vec2::new(5., -5.)],
                stroke,
            );
        }
        HeaderControl::Window(fullscreen) => {
            if fullscreen {
                p.add(egui::Shape::line(
                    vec![
                        center + Vec2::new(-2., -6.),
                        center + Vec2::new(6., -6.),
                        center + Vec2::new(6., 2.),
                    ],
                    stroke,
                ));
                p.rect_stroke(
                    Rect::from_min_max(center + Vec2::new(-6., -2.), center + Vec2::new(2., 6.)),
                    1.,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            } else {
                for (x, y) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
                    p.add(egui::Shape::line(
                        vec![
                            center + Vec2::new(x * 2., y * 6.),
                            center + Vec2::new(x * 6., y * 6.),
                            center + Vec2::new(x * 6., y * 2.),
                        ],
                        stroke,
                    ));
                }
            }
        }
    }
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(label)
}
pub fn title(ui: &mut egui::Ui, kicker: &str, title: &str, sub: &str) {
    eyebrow(ui, kicker);
    ui.add_space(10.);
    ui.label(RichText::new(title).font(display(32.)));
    ui.add_space(2.);
    ui.label(RichText::new(sub).color(MUTED).size(14.));
    ui.add_space(23.);
}
pub fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 12.;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(18., 2.), egui::Sense::hover());
            ui.painter().rect_filled(r, 1., YELLOW);
            ui.label(
                RichText::new(text)
                    .font(strong(10.))
                    .color(YELLOW)
                    .extra_letter_spacing(1.8),
            );
        });
    });
}
pub fn panel(ui: &mut egui::Ui, render: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1_f32, LINE))
        .corner_radius(22)
        .inner_margin(26)
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 2.).max(0.));
            render(ui);
        });
}
pub fn logo(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(36.), egui::Sense::hover());
    ui.painter().rect_filled(r, 11., YELLOW);
    let p = |x: f32, y: f32| r.min + Vec2::new(x, y);
    ui.painter().add(egui::Shape::line(
        vec![
            p(10., 26.),
            p(10., 10.),
            p(14., 10.),
            p(26., 26.),
            p(26., 10.),
            p(20., 10.),
        ],
        Stroke::new(2.3_f32, INK),
    ));
}
pub fn pill(ui: &mut egui::Ui, text: &str, active: bool) {
    egui::Frame::new()
        .fill(if active {
            Color32::from_rgb(34, 35, 18)
        } else {
            SURFACE
        })
        .stroke(Stroke::new(
            1_f32,
            if active {
                Color32::from_rgb(72, 70, 30)
            } else {
                LINE
            },
        ))
        .corner_radius(30)
        .inner_margin(egui::Margin::symmetric(13, 7))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = 12.;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(6.), egui::Sense::hover());
                ui.painter()
                    .circle_filled(r.center(), 3., if active { YELLOW } else { MUTED });
                ui.label(RichText::new(text).font(strong(10.)).color(if active {
                    YELLOW
                } else {
                    MUTED
                }));
            });
        });
}
pub fn rail(ui: &mut egui::Ui, stage: usize) {
    ui.add_space(27.);
    ui.label(
        RichText::new("A TUA VIAGEM")
            .font(strong(9.))
            .extra_letter_spacing(1.6)
            .color(MUTED),
    );
    ui.add_space(23.);
    for (i, (name, sub)) in [
        ("Ler cartão", "O ponto de partida"),
        ("Escolher", "Passe ou saldo"),
        ("Pagamento", "Confirma no MB WAY"),
        ("Carregar", "Pronto para seguir"),
    ]
    .iter()
    .enumerate()
    {
        let active = i == stage;
        let complete = i < stage;
        egui::Frame::new()
            .fill(if active {
                Color32::from_rgb(30, 32, 20)
            } else {
                Color32::TRANSPARENT
            })
            .corner_radius(13)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(27.), egui::Sense::hover());
                    ui.painter().circle_filled(
                        r.center(),
                        13.,
                        if active { YELLOW } else { SURFACE },
                    );
                    if complete {
                        ui.painter().add(egui::Shape::line(
                            vec![
                                r.center() + Vec2::new(-4., 0.),
                                r.center() + Vec2::new(-1., 3.),
                                r.center() + Vec2::new(5., -4.),
                            ],
                            Stroke::new(1.5_f32, YELLOW),
                        ));
                    } else {
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            format!("{:02}", i + 1),
                            strong(10.),
                            if active {
                                INK
                            } else if complete {
                                YELLOW
                            } else {
                                MUTED
                            },
                        );
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(*name).font(strong(12.)).color(if active {
                            YELLOW
                        } else if complete {
                            Color32::WHITE
                        } else {
                            MUTED
                        }));
                        ui.label(RichText::new(*sub).size(9.).color(MUTED));
                    });
                });
            });
        ui.add_space(13.);
    }
    ui.add_space(20.);
    ui.label(
        RichText::new("Do teu cartão\nà próxima paragem.")
            .font(display(12.))
            .color(MUTED),
    );
}
pub fn ambient(painter: &egui::Painter, rect: Rect) {
    let corner = Pos2::new(rect.max.x - 60., rect.min.y + 70.);
    for i in 0..4 {
        painter.circle_stroke(
            corner,
            90. + i as f32 * 45.,
            Stroke::new(1_f32, Color32::from_rgba_unmultiplied(100, 126, 143, 7)),
        );
    }
}
fn gradient_card(p: &egui::Painter, r: Rect) {
    p.rect_filled(
        r.translate(Vec2::new(0., 13.)),
        25.,
        Color32::from_black_alpha(65),
    );
    p.rect_filled(r, 25., YELLOW);
    let clip = p.with_clip_rect(r.shrink(2.));
    for i in 0..5 {
        clip.circle_stroke(
            r.right_bottom() + Vec2::new(34., -22.),
            70. + i as f32 * 25.,
            Stroke::new(1_f32, Color32::from_black_alpha(17)),
        );
    }
}
pub fn pass_art(ui: &mut egui::Ui) {
    let width = ui.available_width().min(406.);
    let (area, response) = ui.allocate_exact_size(Vec2::new(width, 324.), egui::Sense::hover());
    let p = ui.painter_at(area);
    let center = area.center() + Vec2::new(0., -10.);
    for i in 0..3 {
        p.circle_stroke(
            center,
            104. + i as f32 * 38.,
            Stroke::new(1_f32, Color32::from_rgba_unmultiplied(255, 224, 0, 15)),
        );
    }
    let lift = ui
        .ctx()
        .animate_bool(response.id.with("lift"), response.hovered())
        * 5.;
    let r = Rect::from_center_size(center - Vec2::new(0., lift), Vec2::new(width - 28., 230.));
    gradient_card(&p, r);
    p.text(
        r.min + Vec2::new(25., 23.),
        Align2::LEFT_TOP,
        "navegante",
        display(23.),
        INK,
    );
    contactless(&p, r.right_top() + Vec2::new(-29., 35.), INK);
    p.text(
        r.min + Vec2::new(25., 91.),
        Align2::LEFT_TOP,
        "Um cartão.",
        display(25.),
        INK,
    );
    p.text(
        r.min + Vec2::new(25., 124.),
        Align2::LEFT_TOP,
        "Toda a cidade.",
        display(25.),
        INK,
    );
    p.text(
        r.left_bottom() + Vec2::new(25., -23.),
        Align2::LEFT_BOTTOM,
        "O TEU PRÓXIMO DESTINO",
        strong(8.),
        INK.gamma_multiply(0.7),
    );
    p.text(
        r.right_bottom() + Vec2::new(-25., -20.),
        Align2::RIGHT_BOTTOM,
        "Lisboa.",
        display(16.),
        INK,
    );
    p.circle_filled(center + Vec2::new(-146., 130.), 5., YELLOW);
    p.circle_stroke(
        center + Vec2::new(-146., 130.),
        10.,
        Stroke::new(1_f32, Color32::from_rgba_unmultiplied(255, 224, 0, 40)),
    );
}
pub fn contactless(p: &egui::Painter, origin: Pos2, color: Color32) {
    for radius in [5., 10., 15.] {
        let points: Vec<_> = (0..=16)
            .map(|i| {
                let a = -0.9 + i as f32 / 16. * 1.8;
                origin + Vec2::new(a.cos(), a.sin()) * radius
            })
            .collect();
        p.add(egui::Shape::line(points, Stroke::new(1.5_f32, color)));
    }
}
pub fn device(ui: &mut egui::Ui, reading: bool) {
    let (r, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 132.), egui::Sense::hover());
    let p = ui.painter_at(r);
    let c = r.center();
    let reader = Rect::from_center_size(c + Vec2::new(0., 23.), Vec2::new(116., 54.));
    p.rect_filled(reader, 14., Color32::from_rgb(31, 41, 52));
    p.rect_stroke(
        reader,
        14.,
        Stroke::new(1_f32, Color32::from_rgb(62, 75, 88)),
        egui::StrokeKind::Inside,
    );
    let card = Rect::from_center_size(c - Vec2::new(0., 13.), Vec2::new(89., 57.));
    p.rect_filled(card, 10., YELLOW);
    p.text(
        card.center(),
        Align2::CENTER_CENTER,
        "n.",
        display(24.),
        INK,
    );
    p.line_segment(
        [c + Vec2::new(-47., 28.), c + Vec2::new(47., 28.)],
        Stroke::new(3_f32, BG),
    );
    if reading {
        let t = ui.input(|i| i.time) as f32;
        let y = card.min.y + 8. + (t.sin() + 1.) * 19.;
        p.line_segment(
            [
                Pos2::new(card.min.x - 10., y),
                Pos2::new(card.max.x + 10., y),
            ],
            Stroke::new(2_f32, Color32::from_rgb(240, 255, 180)),
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
}
pub fn feature(ui: &mut egui::Ui, number: &str, title: &str, subtitle: &str) {
    ui.spacing_mut().item_spacing.y = 6.;
    ui.label(RichText::new(number).font(strong(10.)).color(YELLOW));
    ui.add_space(3.);
    ui.label(RichText::new(title).font(strong(13.)));
    ui.label(RichText::new(subtitle).size(11.).color(MUTED));
}
