use super::widgets::*;
use super::*;
use egui::{
    Align, Align2, Frame, Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    ViewportCommand,
};

fn navigation_keyboard(ctx: &egui::Context, ids: &[Option<egui::Id>; 8]) {
    if !ctx.input(|input| input.focused) {
        return;
    }
    let escape = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    let focused = ctx.memory(|memory| memory.focused());
    // egui can clear focus before the first frame's lock filter is installed.
    // In that case Escape still belongs to the navigation item focused last frame.
    let index = ids.iter().position(|id| {
        id.is_some()
            && (*id == focused
                || (escape
                    && focused.is_none()
                    && ctx.memory(|memory| memory.had_focus_last_frame(id.unwrap()))))
    });
    let Some(index) = index else { return };
    let key = ctx.input_mut(|input| {
        [
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
            egui::Key::Home,
            egui::Key::End,
            egui::Key::Escape,
        ]
        .into_iter()
        .find(|key| input.consume_key(egui::Modifiers::NONE, *key))
    });
    let Some(key) = key else { return };
    let next = match key {
        egui::Key::ArrowUp => (index + ids.len() - 1) % ids.len(),
        egui::Key::ArrowDown => (index + 1) % ids.len(),
        egui::Key::Home => 0,
        egui::Key::End => ids.len() - 1,
        _ => index,
    };
    if let Some(id) = ids[next] {
        ctx.memory_mut(|memory| {
            memory.request_focus(id);
            // Cancel egui's already queued spatial arrow navigation.
            memory.move_focus(egui::FocusDirection::None);
        });
        ctx.data_mut(|data| {
            data.insert_temp(
                id.with("navigation_tip_dismissed"),
                key == egui::Key::Escape,
            );
        });
    }
}

fn navigation_tooltip(response: &egui::Response, description: &str) -> bool {
    let ctx = &response.ctx;
    let hovered = response.hovered();
    let focused = response.has_focus();
    let gained_focus = response.gained_focus();
    let escape = (hovered || focused)
        && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let dismissed_id = response.id.with("navigation_tip_dismissed");
    let hover_id = response.id.with("navigation_tip_hovered");
    let dismissed = ctx.data_mut(|data| {
        let previously_hovered = data.get_temp::<bool>(hover_id).unwrap_or(false);
        data.insert_temp(hover_id, hovered);
        if gained_focus || (hovered && !previously_hovered) || (!hovered && !focused) {
            data.insert_temp(dismissed_id, false);
        }
        if escape {
            data.insert_temp(dismissed_id, true);
        }
        data.get_temp::<bool>(dismissed_id).unwrap_or(false)
    });
    if focused {
        ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                },
            );
        });
    }
    let open = !dismissed && (hovered || focused);
    if open {
        if focused {
            response.show_tooltip_text(description);
        } else {
            response.clone().on_hover_text(description);
        }
    }
    open
}

fn navigation_icon_rect(rect: Rect, response: &egui::Response, reduce_motion: bool) -> Rect {
    let pressed = response.is_pointer_button_down_on()
        || (response.has_focus()
            && response.ctx.input(|input| {
                input.key_down(egui::Key::Enter) || input.key_down(egui::Key::Space)
            }));
    let scale = if pressed && !reduce_motion { 0.92 } else { 1.0 };
    Rect::from_center_size(
        rect.left_center() + Vec2::new(24.0, 0.0),
        Vec2::splat(20.0 * scale),
    )
}

fn sidebar_toggle(ui: &mut egui::Ui, expanded: bool) -> egui::Response {
    let (_, rect) = ui.allocate_space(Vec2::new(ui.available_width(), CONTROL_HEIGHT));
    let response = ui
        .interact(rect, egui::Id::new("sidebar_toggle"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let p = palette(ui.visuals().dark_mode);
    let highlighted = response.hovered() || response.has_focus();
    let pressed = response.is_pointer_button_down_on();
    ui.painter().rect_filled(
        rect,
        6,
        if pressed {
            p.surface
        } else if highlighted {
            p.soft
        } else {
            p.rail
        },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            6,
            Stroke::new(1.0_f32, p.text),
            egui::StrokeKind::Inside,
        );
    }
    let center = rect.left_center() + Vec2::new(24.0, 0.0);
    let icon = Rect::from_center_size(center, Vec2::new(18.0, 16.0));
    let stroke = Stroke::new(1.3_f32, if highlighted { p.text } else { p.muted });
    ui.painter()
        .rect_stroke(icon, 2, stroke, egui::StrokeKind::Inside);
    ui.painter().line_segment(
        [
            Pos2::new(icon.left() + 5.0, icon.top()),
            Pos2::new(icon.left() + 5.0, icon.bottom()),
        ],
        stroke,
    );
    let direction = if expanded { -1.0 } else { 1.0 };
    let arrow = center + Vec2::new(2.5, 0.0);
    ui.painter().add(egui::Shape::line(
        vec![
            arrow + Vec2::new(-direction * 2.0, -3.0),
            arrow + Vec2::new(direction * 1.0, 0.0),
            arrow + Vec2::new(-direction * 2.0, 3.0),
        ],
        stroke,
    ));
    let label = if expanded {
        "收起菜单"
    } else {
        "展开菜单"
    };
    if expanded {
        ui.painter().text(
            rect.left_center() + Vec2::new(52.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            if highlighted { p.text } else { p.muted },
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(label)
}

impl NativeApp {
    pub(super) fn shell(&mut self, ctx: &egui::Context) {
        let p = palette(self.dark);
        egui::TopBottomPanel::top("window_title")
            .exact_height(34.0)
            .frame(
                Frame::new()
                    .fill(p.rail)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
                ui.spacing_mut().interact_size = Vec2::new(30.0, 26.0);
                ui.horizontal_centered(|ui| {
                    let width = (ui.available_width() - 140.0).max(40.0);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click_and_drag());
                    ui.painter().text(
                        rect.left_center(),
                        Align2::LEFT_CENTER,
                        "ai2tiku · AI 题库",
                        FontId::proportional(12.0),
                        p.muted,
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    if response.double_clicked() {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if ui
                        .add_sized([34.0, 26.0], egui::Button::new("—").frame(false))
                        .on_hover_text("最小化")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                    if ui
                        .add_sized([34.0, 26.0], egui::Button::new("□").frame(false))
                        .on_hover_text("最大化 / 还原")
                        .clicked()
                    {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if ui
                        .add_sized([34.0, 26.0], egui::Button::new("×").frame(false))
                        .on_hover_text("关闭")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });
        let compact = ctx.content_rect().height() < 700.0;
        if self.provider_form.is_none()
            && self.model_form.is_none()
            && self.question_form.is_none()
            && self.confirm.is_none()
        {
            navigation_keyboard(ctx, &self.nav_ids);
        }
        let expanded = self.sidebar_expanded;
        egui::SidePanel::left("navigation")
            .exact_width(if expanded { 200.0 } else { 72.0 })
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(p.rail)
                    .inner_margin(Margin::symmetric(12, if compact { 12 } else { 20 })),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = if compact { 4.0 } else { 6.0 };
                let (_, brand_rect) = ui.allocate_space(Vec2::new(ui.available_width(), 38.0));
                let logo = if self.dark {
                    &self.logo_dark
                } else {
                    &self.logo
                };
                let logo_rect = Rect::from_center_size(
                    brand_rect.left_center() + Vec2::new(24.0, 0.0),
                    Vec2::new(40.0, 25.0),
                );
                ui.painter().image(
                    logo.id(),
                    logo_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                if expanded {
                    ui.painter().text(
                        brand_rect.left_top() + Vec2::new(52.0, 0.0),
                        Align2::LEFT_TOP,
                        "ai2tiku",
                        FontId::proportional(19.0),
                        p.text,
                    );
                    ui.painter().text(
                        brand_rect.left_bottom() + Vec2::new(52.0, -1.0),
                        Align2::LEFT_BOTTOM,
                        "AI 题库",
                        FontId::proportional(12.0),
                        p.muted,
                    );
                }
                let toggle = sidebar_toggle(ui, expanded);
                self.sidebar_toggle_rect = Some(toggle.rect);
                if toggle.clicked() {
                    self.sidebar_expanded = !expanded;
                    ctx.request_repaint();
                }
                ui.add_space(if compact { 4.0 } else { 12.0 });
                let (_, section_rect) = ui.allocate_space(Vec2::new(ui.available_width(), 18.0));
                if expanded {
                    ui.painter().text(
                        section_rect.left_center(),
                        Align2::LEFT_CENTER,
                        "工作空间",
                        FontId::proportional(12.0),
                        p.muted,
                    );
                }
                for (nav_index, page) in Page::ALL.into_iter().enumerate() {
                    if page == Page::Providers || page == Page::Stats {
                        ui.add_space(if compact { 6.0 } else { 10.0 });
                    }
                    let (_, rect) = ui.allocate_space(Vec2::new(
                        ui.available_width(),
                        if compact { 36.0 } else { 44.0 },
                    ));
                    let response = ui
                        .interact(
                            rect,
                            egui::Id::new(("native_navigation", nav_index)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    let active = self.page == page;
                    self.nav_rects[nav_index] = Some(rect);
                    self.nav_ids[nav_index] = Some(response.id);
                    if active || response.hovered() || response.has_focus() {
                        ui.painter().rect_filled(
                            rect,
                            7,
                            if response.is_pointer_button_down_on() {
                                p.line
                            } else if active {
                                p.surface
                            } else {
                                p.soft
                            },
                        );
                    }
                    if response.hovered() || response.has_focus() {
                        ui.painter().rect_stroke(
                            rect.shrink(1.0),
                            7,
                            Stroke::new(1.0_f32, p.muted),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if active {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                rect.left_center() - Vec2::new(0.0, 8.0),
                                Vec2::new(2.0, 16.0),
                            ),
                            1,
                            p.text,
                        );
                    }
                    let focused = response.hovered() || response.has_focus();
                    let progress = if self.reduce_motion {
                        0.0
                    } else {
                        ctx.animate_bool_with_time(
                            response.id,
                            focused,
                            if page == Page::Deploy {
                                1.35
                            } else if page == Page::Settings {
                                0.62
                            } else {
                                0.4
                            },
                        )
                    };
                    self.nav_progress[nav_index] = progress;
                    let icon_rect = navigation_icon_rect(rect, &response, self.reduce_motion);
                    paint::nav_icon(
                        ui.painter(),
                        icon_rect,
                        page,
                        progress,
                        if active || focused { p.text } else { p.muted },
                        ctx.input(|i| i.time),
                    );
                    if page == Page::Deploy && progress > 0.0 && !self.reduce_motion {
                        // Let the native vsync pace continuous exhaust, including fade-out.
                        ctx.request_repaint();
                    }
                    if expanded {
                        ui.painter().text(
                            rect.left_center() + Vec2::new(52.0, 0.0),
                            Align2::LEFT_CENTER,
                            page.title(),
                            FontId::proportional(14.0),
                            if active || focused { p.text } else { p.muted },
                        );
                    }
                    navigation_tooltip(
                        &response,
                        &format!("{}\n{}", page.title(), page.description()),
                    );
                    if response.clicked() {
                        self.page = page;
                        self.refresh(ctx);
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            true,
                            active,
                            page.title(),
                        )
                    });
                }
                ui.with_layout(
                    Layout::bottom_up(if expanded { Align::LEFT } else { Align::Center }),
                    |ui| {
                        muted(ui, format!("v{}", env!("CARGO_PKG_VERSION")));
                        if expanded {
                            ui.label(
                                RichText::new("知识有迹 · 学无止境")
                                    .size(12.0)
                                    .color(p.muted),
                            );
                        }
                        ui.separator();
                    },
                );
            });
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(p.paper)
                    .inner_margin(Margin::symmetric(24, 20)),
            )
            .show(ctx, |ui| {
                paint::background(ui.painter(), ui.max_rect(), self.dark);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(self.page.title());
                        muted(ui, self.page.description());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.selectable_label(self.dark, "夜间").clicked() {
                            self.set_theme(ctx, true);
                        }
                        if ui.selectable_label(!self.dark, "日间").clicked() {
                            self.set_theme(ctx, false);
                        }
                        ui.add_space(12.0);
                        if let Some(snapshot) = &self.snapshot {
                            ui.label(
                                RichText::new(if snapshot.deployment.server.running {
                                    format!("● 运行中 · {}", snapshot.deployment.server.port)
                                } else {
                                    "○ 服务已停止".into()
                                })
                                .size(12.0)
                                .color(p.muted),
                            );
                        }
                    });
                });
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(12.0);
                if let Some(e) = self.refresh_error.clone() {
                    note(ui, &format!("读取失败：{e}"));
                    if button(ui, "重新读取", false).clicked() {
                        self.refresh(ctx);
                    }
                }
                ScrollArea::vertical()
                    .id_salt(("page", self.page as u8))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match self.page {
                            Page::Dashboard => self.dashboard(ui, ctx),
                            Page::Deploy => self.deploy(ui, ctx),
                            Page::Ocs => self.ocs(ui, ctx),
                            Page::Providers => self.providers(ui, ctx),
                            Page::Questions => self.questions(ui, ctx),
                            Page::Records => self.records(ui, ctx),
                            Page::Stats => self.statistics(ui),
                            Page::Settings => self.settings_page(ui, ctx),
                        }
                        ui.add_space(24.0);
                    });
            });
        window_edges(ctx);
        if let Some(n) = &self.notification {
            if n.created.elapsed() < Duration::from_secs(if n.error { 10 } else { 5 }) {
                egui::Area::new("notification".into())
                    .anchor(Align2::RIGHT_BOTTOM, [-24.0, -20.0])
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        Frame::new()
                            .fill(p.surface)
                            .stroke(Stroke::new(1.0_f32, p.text))
                            .corner_radius(8)
                            .inner_margin(14)
                            .show(ui, |ui| {
                                ui.set_max_width(480.0);
                                ui.label(format!(
                                    "{}  {}",
                                    if n.error { "!" } else { "✓" },
                                    n.text
                                ));
                            });
                    });
            }
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark = dark;
        theme(ctx, dark);
        if let Err(e) = self
            .state
            .db
            .set_ui_pref("theme", if dark { "dark" } else { "light" })
        {
            self.notify(e, true);
        }
    }

    fn dashboard(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(s) = self.snapshot.clone() else {
            ui.spinner();
            return;
        };
        let metrics = [
            (
                "今日搜题",
                count(s.stats.today_count),
                format!("{} Token", count(s.stats.today_tokens)),
            ),
            (
                "题库总量",
                count(s.stats.question_total),
                "道已积累题目".into(),
            ),
            (
                "缓存命中率",
                format!("{:.1}%", s.stats.hit_rate * 100.0),
                format!("命中 {} 次", s.stats.cache_hits),
            ),
            (
                "Token 消耗",
                count(s.stats.total_tokens),
                "累计模型用量".into(),
            ),
        ];
        let columns = if ui.available_width() < 760.0 { 2 } else { 4 };
        for chunk in metrics.chunks(columns) {
            ui.columns(columns, |cols| {
                for (i, (label, value, foot)) in chunk.iter().enumerate() {
                    metric(&mut cols[i], label, value.clone(), foot);
                }
            });
            ui.add_space(6.0);
        }
        let draw_service = |this: &mut Self, ui: &mut egui::Ui| {
            card(ui, "本地题库服务", |ui| {
                ui.set_min_height((s.stats.model_usage.len() as f32 * 45.0).max(150.0));
                pair(
                    ui,
                    "当前状态",
                    if s.deployment.server.running {
                        "运行中"
                    } else {
                        "已停止"
                    },
                );
                pair(
                    ui,
                    "接口地址",
                    &format!("{}/api/search", s.deployment.server.address),
                );
                pair(ui, "监听范围", "127.0.0.1 · 仅本机");
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if button(ui, "配置 OCS", true).clicked() {
                        this.page = Page::Ocs;
                    }
                    ui.add_enabled_ui(!this.pending.contains("service"), |ui| {
                        if button(
                            ui,
                            if s.deployment.server.running {
                                "停止服务"
                            } else {
                                "启动服务"
                            },
                            false,
                        )
                        .clicked()
                        {
                            this.toggle_service(ctx);
                        }
                    });
                });
            });
        };
        if ui.available_width() > 760.0 {
            ui.columns(2, |cols| {
                draw_service(self, &mut cols[0]);
                card(&mut cols[1], "模型调用分布", |ui| {
                    ui.set_min_height((s.stats.model_usage.len() as f32 * 45.0).max(150.0));
                    model_bars(ui, &s.stats.model_usage);
                });
            });
        } else {
            draw_service(self, ui);
            ui.add_space(6.0);
            card(ui, "模型调用分布", |ui| {
                model_bars(ui, &s.stats.model_usage);
            });
        }
        ui.add_space(6.0);
        card(ui, "最近搜题", |ui| {
            if s.recent.is_empty() {
                muted(ui, "还没有答题记录。接入 OCS 后，搜题记录会出现在这里。");
            }
            let widths = [5.0, 1.3, 1.2];
            table_header(ui, &widths, &["题目", "来源", "时间"]);
            for r in &s.recent {
                row(ui, &widths, 44.0, |ui, col| match col {
                    0 => {
                        ui.add(egui::Label::new(&r.title).truncate())
                            .on_hover_text(&r.title);
                    }
                    1 => {
                        ui.label(source(&r.source));
                    }
                    _ => muted(ui, date(&r.created_at)),
                });
                ui.separator();
            }
            if button(ui, "查看全部记录", false).clicked() {
                self.page = Page::Records;
            }
        });
    }

    fn deploy(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        note(ui,"安装与权限设置都在 Edge 中完成。软件能读取本地服务和模型状态；是否已接入 OCS，仍需实际搜题验证。");
        ui.add_space(6.0);
        let installed = self.browser.edge_installed;
        for (i,title,description) in [
            (1,"安装脚本猫","在 Edge 加载项商店安装并启用脚本猫扩展。"),
            (2,"开启用户脚本权限","扩展管理 → 脚本猫 → 详细信息 → 开启「允许用户脚本」。旧版或没有此选项时，开启扩展页「开发人员模式」。重新启用扩展或重启浏览器，然后刷新课程页面。"),
            (3,"安装 OCS 脚本","读取官方最新版本，在脚本管理器的安装页面确认。下载成文件或未出现安装页时，先检查上一步权限。"),
            (4,"配置 AI 供应商与模型","填入自己的 API Key，读取模型并选择默认模型；在 OCS 接入页自测。"),
            (5,"复制配置并验证","将完整 JSON 粘贴到 OCS 题库配置并保存。在课程中实际搜题，核对答题记录中的题目与结果。"),
        ] {
            card(ui,"",|ui|{
                ui.horizontal(|ui|{ui.label(RichText::new(format!("{i:02}")).size(23.0).color(palette(self.dark).muted));ui.label(RichText::new(title).size(16.0).strong());});
                ui.label(description);ui.add_space(3.0);
                ui.horizontal_wrapped(|ui|{match i {
                    1=>{muted(ui,if installed{"Edge 已安装"}else{"未检测到 Edge"});if button(ui,"打开 Edge 加载项",true).clicked(){self.open(ctx,"https://microsoftedge.microsoft.com/addons/detail/scriptcat/liilgpjgabokdklappibcjfablkpcekh");}},
                    2=>{if button(ui,"打开扩展管理",true).clicked(){self.open(ctx,"edge://extensions/");}if button(ui,"查看权限设置图解",false).clicked(){self.open(ctx,"https://docs.scriptcat.org/docs/use/open-dev/?browser=edge");}},
                    3=>{if let Some(r)=self.release.clone(){ui.label(format!("OCS {}",r.version));if button(ui,"安装 OCS 脚本",true).clicked(){self.open(ctx,r.script_url);}}if button(ui,if self.pending.contains("release"){"正在读取…"}else{"获取最新版本"},false).clicked(){self.load_release(ctx);}if button(ui,"查看官方发布页",false).clicked(){self.open(ctx,"https://github.com/ocsjs/ocsjs/releases/latest");}},
                    4=>{if let Some(s)=&self.snapshot {if let Some(m)=&s.deployment.ai_model{ui.label(format!("{} · {}{}",m.provider,m.model,if s.deployment.ai_fallback{""}else{" · AI 兜底已关闭"}));}else{muted(ui,"还没有启用的供应商与模型");}}if button(ui,"配置供应商",true).clicked(){self.page=Page::Providers;}},
                    _=>{muted(ui,"待在 OCS 中验证");ui.add_enabled_ui(self.snapshot.as_ref().is_some_and(|s|s.deployment.server.running)&&!self.pending.contains("copy"),|ui|{if button(ui,"复制完整 JSON",true).clicked(){self.copy_config(ctx);}});if button(ui,"接入与自测",false).clicked(){self.page=Page::Ocs;}},
                }});
                if i==3{if let Some(e)=&self.release_error{muted(ui,e);}}
            });ui.add_space(6.0);
        }
    }

    fn ocs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        note(ui,"接入前请确认脚本猫已允许用户脚本；OCS 脚本还需允许访问本地地址。复制的是完整 JSON 内容，请勿粘贴订阅链接。改端口后请重新复制并保存。");
        ui.add_space(6.0);
        card(ui, "本地连接", |ui| {
            if let Some(s) = self.snapshot.clone() {
                pair(
                    ui,
                    "接口地址",
                    &format!("{}/api/search", s.deployment.server.address),
                );
                pair(
                    ui,
                    "当前状态",
                    if s.deployment.server.running {
                        "运行中"
                    } else {
                        "服务未启动"
                    },
                );
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(
                        s.deployment.server.running && !self.pending.contains("copy"),
                        |ui| {
                            if button(ui, "复制题库 JSON", true).clicked() {
                                self.copy_config(ctx);
                            }
                        },
                    );
                    ui.add_enabled_ui(!self.pending.contains("service"), |ui| {
                        if button(
                            ui,
                            if s.deployment.server.running {
                                "停止服务"
                            } else {
                                "启动服务"
                            },
                            false,
                        )
                        .clicked()
                        {
                            self.toggle_service(ctx);
                        }
                    });
                    if button(ui, "刷新状态", false).clicked() {
                        self.refresh(ctx);
                    }
                    if button(ui, "权限设置图解", false).clicked() {
                        self.open(
                            ctx,
                            "https://docs.scriptcat.org/docs/use/open-dev/?browser=edge",
                        );
                    }
                });
                egui::CollapsingHeader::new("查看完整 JSON 配置").show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("config")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            let mut config = s.deployment.config;
                            ui.add(
                                egui::TextEdit::multiline(&mut config)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(true),
                            );
                        });
                });
            } else {
                muted(ui, "连接状态读取中，暂不可复制配置。");
            }
        });
        ui.add_space(6.0);
        card(ui, "搜题自测", |ui| {
            muted(
                ui,
                "这里验证本地缓存和模型调用。浏览器 OCS 是否接入，还需在课程中实际搜题核验。",
            );
            ui.horizontal(|ui| {
                ui.label("题型");
                type_picker(ui, "test_type", &mut self.search_type);
            });
            multiline(ui, "题干", &mut self.search_title, 2);
            multiline(
                ui,
                "选项（每行一个，填空题可留空）",
                &mut self.search_options,
                4,
            );
            ui.horizontal(|ui| {
                ui.add_enabled_ui(
                    !self.pending.contains("search") && !self.search_title.trim().is_empty(),
                    |ui| {
                        if button(
                            ui,
                            if self.pending.contains("search") {
                                "正在搜题…"
                            } else {
                                "开始自测"
                            },
                            true,
                        )
                        .clicked()
                        {
                            let state = self.state.clone();
                            let query = SearchQuery {
                                title: self.search_title.clone(),
                                qtype: self.search_type.clone(),
                                options: self.search_options.clone(),
                                ..Default::default()
                            };
                            self.search_result = None;
                            self.job(ctx, "search", async move {
                                Event::Search(crate::search::answer(&state, &query).await)
                            });
                        }
                    },
                );
                muted(ui, "未命中缓存时会按当前设置调用模型并消耗额度。");
            });
            if let Some(result) = &self.search_result {
                ui.separator();
                if result.code == 1 {
                    ui.label(
                        RichText::new(result.answer.as_deref().unwrap_or_default())
                            .size(22.0)
                            .strong(),
                    );
                    muted(
                        ui,
                        format!(
                            "{} · {} · {} ms · {} Token",
                            source(&result.source),
                            result.model,
                            result.duration_ms,
                            result.tokens
                        ),
                    );
                    if let Some(explanation) = &result.explanation {
                        ui.label(explanation);
                    }
                } else {
                    note(ui, result.msg.as_deref().unwrap_or("搜题失败"));
                }
            }
        });
    }

    fn providers(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(s) = self.snapshot.clone() else {
            ui.spinner();
            return;
        };
        card(ui, "AI 供应商", |ui| {
            ui.horizontal_wrapped(|ui| {
                muted(ui, "兼容 OpenAI 协议；API Key 保存在本机。");
                if button(ui, "＋ 新增供应商", true).clicked() {
                    self.provider_form = Some(ProviderForm {
                        input: ProviderInput {
                            id: None,
                            name: String::new(),
                            base_url: String::new(),
                            api_key: String::new(),
                            enabled: Some(true),
                        },
                        preset: usize::MAX,
                        show_key: false,
                    });
                }
            });
            if s.providers.is_empty() {
                ui.add_space(14.0);
                muted(ui, "还没有供应商。选择预设，填入 Key 后即可读取模型。");
            }
            for provider in &s.providers {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&provider.name).strong());
                    muted(
                        ui,
                        if provider.enabled {
                            "已启用"
                        } else {
                            "已禁用"
                        },
                    );
                    if button(ui, "编辑", false).clicked() {
                        self.provider_form = Some(ProviderForm {
                            input: ProviderInput {
                                id: Some(provider.id),
                                name: provider.name.clone(),
                                base_url: provider.base_url.clone(),
                                api_key: provider.api_key.clone(),
                                enabled: Some(provider.enabled),
                            },
                            preset: usize::MAX,
                            show_key: false,
                        });
                    }
                    ui.add_enabled_ui(!self.pending.contains("sync"), |ui| {
                        if button(ui, "读取模型", false).clicked() {
                            self.sync_models(ctx, provider.id);
                        }
                    });
                    ui.add_enabled_ui(!self.pending.contains("test_provider"), |ui| {
                        if button(ui, "测连通性", false).clicked() {
                            let provider = provider.clone();
                            let state = self.state.clone();
                            self.job(ctx, "test_provider", async move {
                                Event::Notice(
                                    "test_provider",
                                    crate::ai::test_provider(&state.client(), &provider).await,
                                )
                            });
                        }
                    });
                    if button(ui, "删除", false).clicked() {
                        self.confirm = Some(Confirm::DeleteProvider(provider.clone()));
                    }
                });
                muted(ui, &provider.base_url);
            }
        });
        ui.add_space(6.0);
        card(ui, "模型", |ui| {
            ui.horizontal_wrapped(|ui| {
                muted(
                    ui,
                    "优先选择启用供应商下的默认模型，未设默认时使用首个可用模型。",
                );
                ui.add_enabled_ui(!s.providers.is_empty(), |ui| {
                    if button(ui, "＋ 新增模型", true).clicked() {
                        self.model_form = Some(ModelForm {
                            input: ModelInput {
                                id: None,
                                provider_id: s.providers[0].id,
                                model: String::new(),
                                display_name: Some(String::new()),
                                temperature: Some(0.3),
                                max_tokens: Some(2048),
                                enabled: Some(true),
                                is_default: Some(false),
                            },
                        });
                    }
                });
            });
            if s.models.is_empty() {
                ui.add_space(10.0);
                muted(ui, "还没有模型。保存供应商后会尝试自动读取，也可手动添加。");
            }
            for model in &s.models {
                ui.separator();
                let provider = s.providers.iter().find(|p| p.id == model.provider_id);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(if model.display_name.is_empty() {
                            &model.model
                        } else {
                            &model.display_name
                        })
                        .strong(),
                    );
                    muted(
                        ui,
                        format!(
                            "{}{}{}",
                            provider.map_or("未知供应商", |p| p.name.as_str()),
                            if model.is_default { " · 默认" } else { "" },
                            if !model.enabled || provider.is_some_and(|p| !p.enabled) {
                                " · 已禁用"
                            } else {
                                ""
                            }
                        ),
                    );
                    if button(ui, "编辑", false).clicked() {
                        self.model_form = Some(ModelForm {
                            input: ModelInput {
                                id: Some(model.id),
                                provider_id: model.provider_id,
                                model: model.model.clone(),
                                display_name: Some(model.display_name.clone()),
                                temperature: Some(model.temperature),
                                max_tokens: Some(model.max_tokens),
                                enabled: Some(model.enabled),
                                is_default: Some(model.is_default),
                            },
                        });
                    }
                    if button(ui, "删除", false).clicked() {
                        self.confirm = Some(Confirm::DeleteModel(model.clone()));
                    }
                });
                muted(
                    ui,
                    format!(
                        "{} · 温度 {} · 最大输出 {}",
                        model.model, model.temperature, model.max_tokens
                    ),
                );
            }
        });
    }

    fn questions(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(s) = self.snapshot.clone() else {
            ui.spinner();
            return;
        };
        card(ui, "题库管理", |ui| {
            ui.horizontal_wrapped(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query.keyword)
                        .hint_text("搜索题干关键词")
                        .desired_width(240.0)
                        .margin(Margin::symmetric(10, 8)),
                );
                egui::ComboBox::from_id_salt("question_source")
                    .selected_text(if self.query.source.is_empty() {
                        "全部来源"
                    } else {
                        source(&self.query.source)
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.query.source, String::new(), "全部来源");
                        for (v, l) in [("manual", "手动录入"), ("ai", "AI 作答")] {
                            ui.selectable_value(&mut self.query.source, v.into(), l);
                        }
                    });
                if button(ui, "搜索", false).clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.query.questions_page = 1;
                    self.refresh(ctx);
                }
                if button(ui, "＋ 新增题目", true).clicked() {
                    self.question_form = Some(QuestionForm {
                        input: QuestionInput {
                            id: None,
                            title: String::new(),
                            options: Some(String::new()),
                            qtype: Some("single".into()),
                            answer: String::new(),
                        },
                    });
                }
            });
            ui.add_space(4.0);
            let title_width = (ui.available_width() - 48.0 - 138.0 - 60.0 - 60.0 - 78.0).max(100.0);
            let widths = [title_width, 60.0, 60.0, 78.0, 138.0];
            table_header(ui, &widths, &["题干", "答案", "题型", "来源", "操作"]);
            for question in &s.questions.items {
                row(ui, &widths, 48.0, |ui, col| match col {
                    0 => {
                        ui.add(egui::Label::new(&question.title).truncate())
                            .on_hover_text(format!("{}\n{}", question.title, question.options));
                    }
                    1 => {
                        ui.add(egui::Label::new(&question.answer).truncate())
                            .on_hover_text(&question.answer);
                    }
                    2 => {
                        ui.label(qtype(&question.qtype));
                    }
                    3 => {
                        muted(ui, source(&question.source));
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            if button(ui, "编辑", false).clicked() {
                                self.question_form = Some(QuestionForm {
                                    input: QuestionInput {
                                        id: Some(question.id),
                                        title: question.title.clone(),
                                        options: Some(question.options.clone()),
                                        qtype: Some(question.qtype.clone()),
                                        answer: question.answer.clone(),
                                    },
                                });
                            }
                            if button(ui, "删除", false).clicked() {
                                self.confirm = Some(Confirm::DeleteQuestion(question.clone()));
                            }
                        });
                    }
                });
                ui.separator();
            }
            if s.questions.items.is_empty() {
                ui.add_space(18.0);
                muted(ui, "没有符合条件的题目。");
                ui.add_space(18.0);
            }
            ui.horizontal_wrapped(|ui| {
                muted(
                    ui,
                    format!(
                        "筛选结果 {} 道 · 第 {} 页",
                        s.questions.total, s.questions.page
                    ),
                );
                ui.add_enabled_ui(s.questions.page > 1, |ui| {
                    if button(ui, "上一页", false).clicked() {
                        self.query.questions_page -= 1;
                        self.refresh(ctx);
                    }
                });
                ui.add_enabled_ui(s.questions.page * 20 < s.questions.total, |ui| {
                    if button(ui, "下一页", false).clicked() {
                        self.query.questions_page += 1;
                        self.refresh(ctx);
                    }
                });
                if button(ui, "清空整个题库", false).clicked() {
                    self.confirm = Some(Confirm::ClearQuestions(s.stats.question_total));
                }
            });
        });
    }

    fn records(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(s) = self.snapshot.clone() else {
            ui.spinner();
            return;
        };
        card(ui, "答题记录", |ui| {
            ui.horizontal(|ui| {
                muted(ui, format!("共 {} 条记录", s.records.total));
                if button(ui, "刷新", false).clicked() {
                    self.refresh(ctx);
                }
                if button(ui, "清空记录", false).clicked() {
                    self.confirm = Some(Confirm::ClearRecords(s.records.total));
                }
            });
            let widths = [3.3, 1.2, 1.1, 1.5, 1.2, 1.4];
            table_header(
                ui,
                &widths,
                &["题目", "答案", "来源", "模型", "耗时", "时间"],
            );
            for r in &s.records.items {
                row(ui, &widths, 48.0, |ui, col| {
                    let text = match col {
                        0 => r.title.clone(),
                        1 => {
                            if r.answer.is_empty() {
                                "未得到答案".into()
                            } else {
                                r.answer.clone()
                            }
                        }
                        2 => source(&r.source).into(),
                        3 => {
                            if r.model.is_empty() {
                                "—".into()
                            } else {
                                r.model.clone()
                            }
                        }
                        4 => format!("{} ms", r.duration_ms),
                        _ => date(&r.created_at),
                    };
                    ui.add(egui::Label::new(&text).truncate())
                        .on_hover_text(if col == 0 {
                            format!("{}\n答案：{}\nToken：{}", r.title, r.answer, r.total_tokens)
                        } else {
                            text
                        });
                });
                ui.separator();
            }
            if s.records.items.is_empty() {
                ui.add_space(18.0);
                muted(ui, "还没有答题记录。");
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(s.records.page > 1, |ui| {
                    if button(ui, "上一页", false).clicked() {
                        self.query.records_page -= 1;
                        self.refresh(ctx);
                    }
                });
                muted(ui, format!("第 {} 页", s.records.page));
                ui.add_enabled_ui(s.records.page * 20 < s.records.total, |ui| {
                    if button(ui, "下一页", false).clicked() {
                        self.query.records_page += 1;
                        self.refresh(ctx);
                    }
                });
            });
        });
    }

    fn statistics(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.snapshot.clone() else {
            ui.spinner();
            return;
        };
        ui.columns(3, |cols| {
            metric(
                &mut cols[0],
                "AI 作答次数",
                count(s.stats.ai_calls),
                "通过模型返回的记录",
            );
            metric(
                &mut cols[1],
                "缓存命中",
                count(s.stats.cache_hits),
                "复用本地答案",
            );
            metric(
                &mut cols[2],
                "缓存命中率",
                format!("{:.1}%", s.stats.hit_rate * 100.0),
                "已记录请求的命中比例",
            );
        });
        ui.add_space(6.0);
        card(ui, "近 14 天搜题量", |ui| {
            chart(
                ui,
                &s.stats
                    .trend
                    .iter()
                    .map(|p| (p.date.clone(), p.count as f32))
                    .collect::<Vec<_>>(),
            );
        });
        ui.add_space(6.0);
        card(ui, "近 14 天 Token 消耗", |ui| {
            chart(
                ui,
                &s.stats
                    .trend
                    .iter()
                    .map(|p| (p.date.clone(), p.tokens as f32))
                    .collect::<Vec<_>>(),
            );
        });
        ui.add_space(6.0);
        card(ui, "按模型统计", |ui| {
            model_bars(ui, &s.stats.model_usage);
        });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        card(ui, "界面", |ui| {
            ui.horizontal(|ui| {
                ui.label("水墨黑白");
                if ui.selectable_label(!self.dark, "日间").clicked() {
                    self.set_theme(ctx, false);
                }
                if ui.selectable_label(self.dark, "夜间").clicked() {
                    self.set_theme(ctx, true);
                }
            });
            if ui
                .checkbox(&mut self.reduce_motion, "减少动态效果")
                .changed()
            {
                if let Err(e) = self.state.db.set_ui_pref(
                    "native_reduce_motion",
                    if self.reduce_motion { "true" } else { "false" },
                ) {
                    self.notify(e, true);
                }
            }
        });
        ui.add_space(6.0);
        card(ui, "服务与答题", |ui| {
            ui.horizontal(|ui| {
                ui.label("监听端口");
                ui.add(egui::DragValue::new(&mut self.settings.port).range(1024..=65535));
            });
            muted(
                ui,
                "更换端口前先尝试绑定；失败时保留原服务与配置。端口变更后需更新 OCS JSON。",
            );
            ui.checkbox(
                &mut self.settings.auto_cache,
                "自动缓存通过校验的文本题答案",
            );
            ui.checkbox(
                &mut self.settings.ai_fallback,
                "本地题库未命中时，调用 AI 作答",
            );
            note(ui,"自动答题使用精确匹配：完整核对题干、题型和选项，再映射乱序答案；不使用字符相似度直接作答。");
            field(
                ui,
                "网络代理（留空为强制直连）",
                &mut self.settings.proxy_url,
                false,
            );
            multiline(
                ui,
                "自定义系统提示词（留空使用内置规则）",
                &mut self.settings.system_prompt,
                5,
            );
            egui::CollapsingHeader::new("查看内置提示词").show(ui, |ui| {
                ui.label(crate::ai::default_system_prompt());
            });
            ui.add_enabled_ui(!self.pending.contains("settings"), |ui| {
                if button(ui, "保存设置", true).clicked() {
                    let state = self.state.clone();
                    let settings = self.settings.clone();
                    self.job(ctx, "settings", async move {
                        Event::Notice(
                            "settings",
                            tokio::task::spawn_blocking(move || {
                                server::apply_settings(&state, settings)
                                    .map(|_| "设置已保存".into())
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string())),
                        )
                    });
                }
            });
        });
    }
}

fn window_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    use egui::{CursorIcon as C, ResizeDirection as D};
    let r = ctx.content_rect();
    let w = 4.0;
    let c = 10.0;
    let edges = [
        (
            Rect::from_min_max(
                r.min + Vec2::new(c, 0.0),
                Pos2::new(r.right() - c, r.top() + w),
            ),
            D::North,
            C::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.left() + c, r.bottom() - w),
                r.max - Vec2::new(c, 0.0),
            ),
            D::South,
            C::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                r.min + Vec2::new(0.0, c),
                Pos2::new(r.left() + w, r.bottom() - c),
            ),
            D::West,
            C::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.right() - w, r.top() + c),
                r.max - Vec2::new(0.0, c),
            ),
            D::East,
            C::ResizeHorizontal,
        ),
        (
            Rect::from_min_size(r.min, Vec2::splat(c)),
            D::NorthWest,
            C::ResizeNwSe,
        ),
        (
            Rect::from_min_size(Pos2::new(r.right() - c, r.top()), Vec2::splat(c)),
            D::NorthEast,
            C::ResizeNeSw,
        ),
        (
            Rect::from_min_size(Pos2::new(r.left(), r.bottom() - c), Vec2::splat(c)),
            D::SouthWest,
            C::ResizeNeSw,
        ),
        (
            Rect::from_min_size(r.max - Vec2::splat(c), Vec2::splat(c)),
            D::SouthEast,
            C::ResizeNwSe,
        ),
    ];
    for (index, (rect, direction, cursor)) in edges.into_iter().enumerate() {
        egui::Area::new(egui::Id::new(("window_edge", index)))
            .fixed_pos(rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());
                let response = response.on_hover_cursor(cursor);
                if response.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                }
            });
    }
}

fn model_bars(ui: &mut egui::Ui, items: &[ModelUsage]) {
    if items.is_empty() {
        muted(ui, "尚无模型调用记录");
        return;
    }
    let max = items.iter().map(|i| i.count).max().unwrap_or(1).max(1) as f32;
    let p = palette(ui.visuals().dark_mode);
    for item in items {
        pair(
            ui,
            &item.model,
            &format!("{} 次 · {} Token", item.count, count(item.tokens)),
        );
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 5.0), Sense::hover());
        ui.painter().rect_filled(rect, 2, p.soft);
        ui.painter().rect_filled(
            Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * item.count as f32 / max, 5.0),
            ),
            2,
            p.text,
        );
        ui.add_space(5.0);
    }
}

fn chart(ui: &mut egui::Ui, points: &[(String, f32)]) {
    let p = palette(ui.visuals().dark_mode);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 180.0), Sense::hover());
    if points.is_empty() {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "暂无数据",
            FontId::proportional(13.0),
            p.muted,
        );
        return;
    }
    let plot = rect.shrink2(Vec2::new(36.0, 24.0));
    let max = points.iter().map(|(_, n)| *n).fold(1.0, f32::max);
    for i in 0..4 {
        let y = plot.bottom() - plot.height() * i as f32 / 3.0;
        ui.painter().line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0_f32, p.line),
        );
        ui.painter().text(
            Pos2::new(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            format!("{:.0}", max * i as f32 / 3.0),
            FontId::proportional(10.0),
            p.muted,
        );
    }
    let mut line = Vec::new();
    for (i, (date, value)) in points.iter().enumerate() {
        let x = if points.len() == 1 {
            plot.center().x
        } else {
            plot.left() + plot.width() * i as f32 / (points.len() - 1) as f32
        };
        let at = Pos2::new(x, plot.bottom() - plot.height() * value / max);
        line.push(at);
        ui.painter().circle_filled(at, 2.5, p.text);
        if i == 0 || i == points.len() - 1 || points.len() < 5 {
            ui.painter().text(
                Pos2::new(x, plot.bottom() + 14.0),
                Align2::CENTER_CENTER,
                date.get(5..).unwrap_or(date),
                FontId::proportional(10.0),
                p.muted,
            );
        }
    }
    ui.painter()
        .add(egui::Shape::line(line, Stroke::new(1.8_f32, p.text)));
}
