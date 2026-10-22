use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui, Vec2};

pub const CONTROL_HEIGHT: f32 = 34.0;

#[derive(Clone, Copy)]
pub struct Palette {
    pub paper: Color32,
    pub surface: Color32,
    pub rail: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub soft: Color32,
}
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            paper: Color32::from_gray(20),
            surface: Color32::from_gray(29),
            rail: Color32::from_gray(17),
            text: Color32::from_gray(238),
            muted: Color32::from_gray(170),
            line: Color32::from_gray(54),
            soft: Color32::from_gray(39),
        }
    } else {
        Palette {
            paper: Color32::from_gray(246),
            surface: Color32::WHITE,
            rail: Color32::from_gray(240),
            text: Color32::from_gray(29),
            muted: Color32::from_gray(102),
            line: Color32::from_gray(220),
            soft: Color32::from_gray(235),
        }
    }
}

pub fn theme(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.override_text_color = Some(p.text);
    v.panel_fill = p.paper;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0_f32, p.line);
    v.extreme_bg_color = p.paper;
    v.faint_bg_color = p.soft;
    v.code_bg_color = p.soft;
    v.selection.bg_fill = p.soft;
    v.selection.stroke = Stroke::new(1.5_f32, p.text);
    v.hyperlink_color = p.text;
    v.warn_fg_color = p.text;
    v.error_fg_color = p.text;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = p.surface;
        w.weak_bg_fill = p.surface;
        w.bg_stroke = Stroke::new(1.0_f32, p.line);
        w.fg_stroke = Stroke::new(1.3_f32, p.text);
        w.corner_radius = CornerRadius::same(6);
        // Feedback changes ink and borders, never control bounds or text alignment.
        w.expansion = 0.0;
    }
    v.widgets.hovered.bg_fill = p.soft;
    v.widgets.hovered.weak_bg_fill = p.soft;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.muted);
    let pressed_fill = Color32::from_gray(if dark { 54 } else { 220 });
    v.widgets.active.bg_fill = pressed_fill;
    v.widgets.active.weak_bg_fill = pressed_fill;
    v.widgets.active.bg_stroke = Stroke::new(2.0_f32, p.text);
    v.widgets.open.bg_fill = p.soft;
    v.widgets.open.weak_bg_fill = p.soft;
    v.widgets.open.bg_stroke = Stroke::new(1.5_f32, p.muted);
    let target = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(target);
    ctx.set_visuals_of(target, v);
    ctx.all_styles_mut(|style| {
        style.spacing.interact_size.y = CONTROL_HEIGHT;
        style.spacing.button_padding = Vec2::new(14.0, 8.0);
    });
}

/// egui 0.33 does not apply Visuals::interact_cursor to every widget itself.
/// Call after drawing the shell and dialogs. Keep text, drag and resize cursors.
pub fn interactive_cursor(ctx: &egui::Context) {
    if ctx.output(|output| output.cursor_icon != egui::CursorIcon::Default) {
        return;
    }
    let hovered =
        ctx.interaction_snapshot(|snapshot| snapshot.hovered.iter().copied().collect::<Vec<_>>());
    if hovered.into_iter().any(|id| {
        ctx.read_response(id).is_some_and(|response| {
            response.enabled() && response.hovered() && response.sense.senses_click()
        })
    }) {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

pub fn card<R>(
    ui: &mut Ui,
    title: &str,
    body: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let p = palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.line))
        .corner_radius(10)
        .inner_margin(18)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(RichText::new(title).size(14.0).strong());
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(6.0);
            }
            body(ui)
        })
}

pub fn button(ui: &mut Ui, text: &str, primary: bool) -> egui::Response {
    ui.scope(|ui| {
        if primary {
            let dark = ui.visuals().dark_mode;
            let p = palette(dark);
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(p.surface);
            for state in [
                &mut visuals.widgets.noninteractive,
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
            ] {
                state.bg_fill = p.text;
                state.weak_bg_fill = p.text;
                state.bg_stroke = Stroke::new(1.0_f32, p.text);
                state.fg_stroke.color = p.surface;
            }
            let hover = Color32::from_gray(if dark { 214 } else { 48 });
            visuals.widgets.hovered.bg_fill = hover;
            visuals.widgets.hovered.weak_bg_fill = hover;
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.muted);
            let pressed = Color32::from_gray(if dark { 190 } else { 72 });
            visuals.widgets.active.bg_fill = pressed;
            visuals.widgets.active.weak_bg_fill = pressed;
            visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, p.text);
        }
        ui.add(egui::Button::new(text).min_size(Vec2::new(64.0, CONTROL_HEIGHT)))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into())
            .size(12.0)
            .color(palette(ui.visuals().dark_mode).muted),
    );
}
pub fn note(ui: &mut Ui, text: &str) {
    let p = palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(p.soft)
        .corner_radius(7)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).size(13.0).color(p.text));
        });
}
pub fn metric(ui: &mut Ui, label: &str, value: String, foot: &str) {
    card(ui, "", |ui| {
        muted(ui, label);
        ui.add_space(4.0);
        ui.label(RichText::new(value).size(29.0).strong());
        muted(ui, foot);
    });
}
pub fn field(ui: &mut Ui, label: &str, value: &mut String, password: bool) {
    ui.label(RichText::new(label).size(13.0).strong());
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(f32::INFINITY)
            .min_size(Vec2::new(0.0, CONTROL_HEIGHT))
            .password(password)
            .margin(Margin::symmetric(10, 8)),
    );
}
pub fn multiline(ui: &mut Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(RichText::new(label).size(13.0).strong());
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_width(f32::INFINITY)
            .desired_rows(rows)
            .margin(Margin::symmetric(10, 8)),
    );
}
pub fn pair(ui: &mut Ui, label: &str, value: &str) {
    let previous = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = 20.0;
    ui.horizontal(|ui| {
        muted(ui, label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(value);
        });
    });
    ui.spacing_mut().interact_size.y = previous;
}

pub fn modal(ctx: &egui::Context, id: &str) -> egui::Modal {
    let p = palette(ctx.style().visuals.dark_mode);
    egui::Modal::new(egui::Id::new(id)).frame(
        Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.line))
            .corner_radius(10)
            .inner_margin(20),
    )
}

pub fn count(n: i64) -> String {
    if n >= 10000 {
        format!("{:.1}万", n as f64 / 10000.0)
    } else {
        n.to_string()
    }
}
pub fn source(s: &str) -> &str {
    match s {
        "cache" => "本地缓存",
        "ai" => "AI 作答",
        "manual" => "手动录入",
        _ => s,
    }
}
pub fn qtype(s: &str) -> &str {
    match s {
        "single" => "单选",
        "multiple" => "多选",
        "judgement" => "判断",
        "completion" => "填空",
        _ => "未知",
    }
}
pub fn type_picker(ui: &mut Ui, id: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(qtype(value))
        .show_ui(ui, |ui| {
            for (v, l) in [
                ("single", "单选"),
                ("multiple", "多选"),
                ("judgement", "判断"),
                ("completion", "填空"),
            ] {
                ui.selectable_value(value, v.to_string(), l);
            }
        });
}
pub fn date(s: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|d| d.format("%m-%d %H:%M").to_string())
                .unwrap_or_else(|_| s.replace('T', " ").chars().take(16).collect())
        })
}

pub fn table_header(ui: &mut Ui, widths: &[f32], labels: &[&str]) {
    row(ui, widths, 32.0, |ui, index| {
        muted(ui, labels[index]);
    });
    ui.separator();
}
pub fn row(ui: &mut Ui, widths: &[f32], height: f32, mut cell: impl FnMut(&mut Ui, usize)) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let sum: f32 = widths.iter().sum();
    let gap = 12.0;
    let total = (rect.width() - gap * (widths.len() - 1) as f32).max(1.0);
    let mut x = rect.left();
    for (index, fraction) in widths.iter().enumerate() {
        let width = total * fraction / sum;
        let cell_rect =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(width, height));
        let clip = ui.clip_rect().intersect(cell_rect);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(cell_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_clip_rect(clip);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                cell(ui, index);
            },
        );
        x += width + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        primary: bool,
        enabled: bool,
        events: Vec<egui::Event>,
    ) -> (egui::Response, Color32, egui::CursorIcon) {
        let mut response = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(400.0, 180.0),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    response = Some(
                        ui.add_enabled_ui(enabled, |ui| button(ui, "Save", primary))
                            .inner,
                    );
                });
                interactive_cursor(ctx);
            },
        );
        let response = response.unwrap();
        let fill = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Rect(rect) = &shape.shape {
                    if rect.rect.contains(response.rect.center())
                        && rect.rect.width() <= response.rect.width() + 2.0
                        && rect.rect.height() <= response.rect.height() + 2.0
                        && rect.fill.a() > 0
                    {
                        return Some(rect.fill);
                    }
                }
                None
            })
            .expect("button must paint a visible background");
        (response, fill, output.platform_output.cursor_icon)
    }

    #[test]
    fn button_pointer_and_keyboard_feedback_changes_ink_without_changing_bounds() {
        for dark in [false, true] {
            for primary in [false, true] {
                let ctx = egui::Context::default();
                theme(&ctx, dark);
                frame(&ctx, primary, true, vec![]);
                let (idle, idle_fill, _) = frame(&ctx, primary, true, vec![]);
                assert_eq!(idle.rect.height(), CONTROL_HEIGHT);
                let center = idle.rect.center();
                let (hover, hover_fill, cursor) =
                    frame(&ctx, primary, true, vec![egui::Event::PointerMoved(center)]);
                assert_eq!(hover.rect, idle.rect);
                assert_ne!(hover_fill, idle_fill, "dark={dark}, primary={primary}");
                assert_eq!(cursor, egui::CursorIcon::PointingHand);
                let (pressed, pressed_fill, _) = frame(
                    &ctx,
                    primary,
                    true,
                    vec![egui::Event::PointerButton {
                        pos: center,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                assert_eq!(pressed.rect, idle.rect);
                assert_ne!(pressed_fill, hover_fill);
                frame(
                    &ctx,
                    primary,
                    true,
                    vec![egui::Event::PointerButton {
                        pos: center,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                ctx.memory_mut(|memory| memory.request_focus(idle.id));
                let (focused, focus_fill, _) =
                    frame(&ctx, primary, true, vec![egui::Event::PointerGone]);
                assert!(focused.has_focus());
                assert_eq!(focused.rect, idle.rect);
                assert_ne!(focus_fill, idle_fill);
            }
        }
    }

    #[test]
    fn disabled_buttons_do_not_advertise_a_click_and_text_keeps_its_cursor() {
        let ctx = egui::Context::default();
        theme(&ctx, false);
        frame(&ctx, true, false, vec![]);
        let (disabled, idle_fill, _) = frame(&ctx, true, false, vec![]);
        let (_, hover_fill, cursor) = frame(
            &ctx,
            true,
            false,
            vec![egui::Event::PointerMoved(disabled.rect.center())],
        );
        assert_eq!(hover_fill, idle_fill);
        assert_ne!(cursor, egui::CursorIcon::PointingHand);
        let mut text = "Editable".to_owned();
        let mut rect = egui::Rect::NOTHING;
        for pass in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(400.0, 180.0),
                    )),
                    events: if pass == 0 {
                        vec![egui::Event::PointerGone]
                    } else {
                        vec![egui::Event::PointerMoved(rect.center())]
                    },
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = ui
                            .add(
                                egui::TextEdit::singleline(&mut text)
                                    .min_size(Vec2::new(0.0, CONTROL_HEIGHT)),
                            )
                            .rect;
                    });
                    interactive_cursor(ctx);
                },
            );
            if pass == 2 {
                assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::Text);
            }
        }
    }
}
