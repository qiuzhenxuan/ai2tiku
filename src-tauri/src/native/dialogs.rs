use super::widgets::*;
use super::*;
use egui::{RichText, ScrollArea};

impl NativeApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut form) = self.provider_form.take() {
            let mut close = false;
            let busy = self.pending.contains("save_provider");
            let response = modal(ctx, "provider_editor").show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 80.0).clamp(300.0, 520.0));
                ui.heading(if form.input.id.is_some() {
                    "编辑供应商"
                } else {
                    "新增供应商"
                });
                ScrollArea::vertical()
                    .id_salt("provider_form_scroll")
                    .max_height((ctx.content_rect().height() - 180.0).max(240.0))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!busy, |ui| {
                            let presets = crate::presets::list();
                            egui::ComboBox::from_id_salt("provider_preset")
                                .selected_text(
                                    presets
                                        .get(form.preset)
                                        .map_or("自定义 OpenAI 兼容服务", |p| p.name),
                                )
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    for (i, preset) in presets.iter().enumerate() {
                                        if ui
                                            .selectable_label(form.preset == i, preset.name)
                                            .clicked()
                                        {
                                            form.preset = i;
                                            form.input.name = preset.name.into();
                                            form.input.base_url = preset.base_url.into();
                                        }
                                    }
                                });
                            field(ui, "名称", &mut form.input.name, false);
                            field(ui, "API 地址", &mut form.input.base_url, false);
                            field(ui, "API Key", &mut form.input.api_key, !form.show_key);
                            ui.checkbox(&mut form.show_key, "显示 Key");
                            let enabled = form.input.enabled.get_or_insert(true);
                            ui.checkbox(enabled, "启用供应商");
                            if let Some(preset) = presets.get(form.preset) {
                                muted(ui, preset.note);
                                if button(ui, "打开供应商平台", false).clicked() {
                                    self.open(ctx, preset.homepage);
                                }
                            }
                        });
                    });
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if button(ui, "保存并读取模型", true).clicked() {
                            let input = form.input.clone();
                            let state = self.state.clone();
                            self.job(ctx, "save_provider", async move {
                                Event::ProviderSaved(
                                    tokio::task::spawn_blocking(move || {
                                        crate::services::save_provider(&state, &input)
                                            .map(|id| (id, input.name))
                                    })
                                    .await
                                    .unwrap_or_else(|e| Err(e.to_string())),
                                )
                            });
                        }
                        if button(ui, "取消", false).clicked() {
                            close = true;
                        }
                    });
                    if busy {
                        ui.spinner();
                    }
                });
            });
            if (!close && !response.should_close()) || busy {
                self.provider_form = Some(form);
            }
        }
        if let Some(mut form) = self.model_form.take() {
            let mut close = false;
            let busy = self.pending.contains("save_model");
            let response = modal(ctx, "model_editor").show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 80.0).clamp(300.0, 520.0));
                ui.heading(if form.input.id.is_some() {
                    "编辑模型"
                } else {
                    "新增模型"
                });
                ScrollArea::vertical()
                    .id_salt("model_form_scroll")
                    .max_height((ctx.content_rect().height() - 180.0).max(240.0))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!busy, |ui| {
                            let providers = self
                                .snapshot
                                .as_ref()
                                .map(|s| s.providers.clone())
                                .unwrap_or_default();
                            ui.label("供应商");
                            egui::ComboBox::from_id_salt("model_provider")
                                .selected_text(
                                    providers
                                        .iter()
                                        .find(|p| p.id == form.input.provider_id)
                                        .map_or("请选择供应商", |p| p.name.as_str()),
                                )
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    for p in &providers {
                                        ui.selectable_value(
                                            &mut form.input.provider_id,
                                            p.id,
                                            &p.name,
                                        );
                                    }
                                });
                            field(ui, "模型标识", &mut form.input.model, false);
                            field(
                                ui,
                                "显示名称（可选）",
                                form.input.display_name.get_or_insert_with(String::new),
                                false,
                            );
                            ui.horizontal(|ui| {
                                ui.label("温度");
                                ui.add(
                                    egui::DragValue::new(form.input.temperature.get_or_insert(0.3))
                                        .range(0.0..=2.0)
                                        .speed(0.05),
                                );
                                ui.label("最大输出 Token");
                                ui.add(
                                    egui::DragValue::new(form.input.max_tokens.get_or_insert(2048))
                                        .range(1..=131072),
                                );
                            });
                            ui.checkbox(form.input.enabled.get_or_insert(true), "启用模型");
                            ui.checkbox(form.input.is_default.get_or_insert(false), "设为默认模型");
                        });
                    });
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if button(ui, "保存模型", true).clicked() {
                            let input = form.input.clone();
                            let state = self.state.clone();
                            self.job(ctx, "save_model", async move {
                                Event::ModelSaved(
                                    tokio::task::spawn_blocking(move || {
                                        crate::services::save_model(&state, &input)
                                            .map(|_| "模型已保存".into())
                                    })
                                    .await
                                    .unwrap_or_else(|e| Err(e.to_string())),
                                )
                            });
                        }
                        if button(ui, "取消", false).clicked() {
                            close = true;
                        }
                    });
                    if busy {
                        ui.spinner();
                    }
                });
            });
            if (!close && !response.should_close()) || busy {
                self.model_form = Some(form);
            }
        }
        if let Some(mut form) = self.question_form.take() {
            let mut close = false;
            let busy = self.pending.contains("save_question");
            let response=modal(ctx,"question_editor").show(ctx,|ui|{
                ui.set_width((ctx.content_rect().width()-80.0).clamp(300.0,560.0));ui.heading(if form.input.id.is_some(){"编辑题目"}else{"新增题目"});
                ScrollArea::vertical().id_salt("question_form_scroll").max_height((ctx.content_rect().height()-180.0).max(240.0)).show(ui,|ui|{
                    ui.add_enabled_ui(!busy,|ui|{
                        ui.horizontal(|ui|{ui.label("题型");type_picker(ui,"question_type",form.input.qtype.get_or_insert_with(||"single".into()));});
                        multiline(ui,"题干",&mut form.input.title,3);multiline(ui,"选项（每行一项）",form.input.options.get_or_insert_with(String::new),4);field(ui,"答案",&mut form.input.answer,false);
                        muted(ui,"单选 A；多选 A#C；判断 true / false；多空答案用 # 分隔。请核对题型与完整选项。");
                    });
                });
                ui.horizontal(|ui|{ui.add_enabled_ui(!busy,|ui|{if button(ui,"保存题目",true).clicked(){let input=form.input.clone();let state=self.state.clone();self.job(ctx,"save_question",async move {Event::QuestionSaved(tokio::task::spawn_blocking(move||crate::services::save_question(&state,&input).map(|_|"题目已保存".into())).await.unwrap_or_else(|e|Err(e.to_string())))});}if button(ui,"取消",false).clicked(){close=true;}});if busy{ui.spinner();}});
            });
            if (!close && !response.should_close()) || busy {
                self.question_form = Some(form);
            }
        }
        if let Some(confirm) = self.confirm.clone() {
            let mut close = false;
            let response=modal(ctx,"confirm_delete").show(ctx,|ui|{
                ui.set_width((ctx.content_rect().width()-80.0).clamp(300.0,460.0));ui.heading("确认删除");
                let text=match &confirm{
                    Confirm::DeleteProvider(p)=>format!("删除供应商「{}」及其全部模型？",p.name),Confirm::DeleteModel(m)=>format!("删除模型「{}」？",m.model),Confirm::DeleteQuestion(q)=>format!("删除题目「{}」？",q.title),
                    Confirm::ClearQuestions(n)=>format!("清空整个题库？当前全库共 {} 道题目，将删除全部题目，包括筛选结果以外的题目。",self.snapshot.as_ref().map_or(*n,|s|s.stats.question_total)),
                    Confirm::ClearRecords(n)=>format!("清空全部 {n} 条答题记录？统计数据也会随之变化。"),
                };
                ui.label(RichText::new(text).size(14.0));muted(ui,"此操作无法撤销。");
                ui.horizontal(|ui|{ui.add_enabled_ui(!self.pending.contains("delete"),|ui|{if button(ui,"确认删除",true).clicked(){let state=self.state.clone();let action=confirm.clone();self.job(ctx,"delete",async move{Event::Notice("delete",tokio::task::spawn_blocking(move||{
                        match action{Confirm::DeleteProvider(p)=>state.db.delete_provider(p.id),Confirm::DeleteModel(m)=>state.db.delete_model(m.id),Confirm::DeleteQuestion(q)=>state.db.delete_question(q.id),Confirm::ClearQuestions(_)=>state.db.clear_questions(),Confirm::ClearRecords(_)=>state.db.clear_records()}?;Ok("已删除".into())
                    }).await.unwrap_or_else(|e|Err(e.to_string())))});close=true;}
                    if button(ui,"取消",false).clicked(){close=true;}
                });});
            });
            if close || response.should_close() {
                self.confirm = None;
            }
        }
    }
}
