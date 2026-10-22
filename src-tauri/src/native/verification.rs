//! Explicit CLI smoke test: isolated database, native framebuffer captures, no model calls.
use super::*;

impl NativeApp {
    pub(super) fn verification_input(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let Some(smoke) = self.smoke.as_ref() else {
            return;
        };
        // Test focus belongs to this viewport even if the user is working in another app.
        // This changes only injected test input, never the operating system's foreground window.
        input.focused = true;
        input.events.retain(|e| {
            !matches!(
                e,
                egui::Event::PointerMoved(_)
                    | egui::Event::PointerButton { .. }
                    | egui::Event::PointerGone
                    | egui::Event::Key { .. }
                    | egui::Event::Text(_)
                    | egui::Event::MouseWheel { .. }
            )
        });
        let stage = smoke.stage;
        self.sidebar_expanded = stage >= 61;
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        let hovered = if (28..52).contains(&stage) && ((stage - 28) % 3 == 1) {
            Some((stage - 28) / 3)
        } else if stage == 60 {
            Some(1)
        } else {
            None
        };
        if let Some(index) = hovered {
            if let Some(rect) = self.nav_rects[index] {
                input.events.push(egui::Event::PointerMoved(rect.center()));
            }
        } else {
            input.events.push(egui::Event::PointerGone);
        }
        if (52..60).contains(&stage) {
            if let Some(id) = self.nav_ids[stage - 52] {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        self.reduce_motion = stage == 60;
    }

    pub(super) fn smoke_tick(&mut self, ctx: &egui::Context) {
        let Some(mut smoke) = self.smoke.take() else {
            return;
        };
        if let Err(error) = std::fs::create_dir_all(&smoke.dir) {
            eprintln!("{error}");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let captures = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        if smoke.waiting && !captures.is_empty() {
            for image in captures {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let file = smoke.dir.join(format!("native-{:02}.png", smoke.stage));
                if let Err(error) = image::save_buffer(
                    &file,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("{error}");
                }
            }
            smoke.stage += 1;
            smoke.frame = 0;
            smoke.waiting = false;
            smoke.stage_started = Instant::now();
        }
        if smoke.stage >= 71 {
            let _ = std::fs::write(
                smoke.dir.join("result.json"),
                serde_json::to_string_pretty(&serde_json::json!({"native":true,"captures":71,"pages":8,"dialogs":4,"themes":2,"sidebarStates":["collapsed","expanded"],"minWindow":[900,600],"data":"isolated demo database","paidModelCalls":0,"motion":smoke.motions})).unwrap_or_default(),
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if !smoke.waiting {
            self.page = if (61..69).contains(&smoke.stage) {
                Page::ALL[smoke.stage - 61]
            } else if smoke.stage >= 28 {
                Page::Dashboard
            } else if smoke.stage >= 24 {
                if smoke.stage < 26 {
                    Page::Providers
                } else {
                    Page::Questions
                }
            } else {
                Page::ALL[smoke.stage % 8]
            };
            let dark = (8..16).contains(&smoke.stage) || smoke.stage == 69;
            if self.dark != dark {
                self.dark = dark;
                widgets::theme(ctx, dark);
            }
            if smoke.frame == 0 {
                self.provider_form = None;
                self.model_form = None;
                self.question_form = None;
                self.confirm = None;
                match smoke.stage {
                    24 => {
                        self.provider_form = Some(ProviderForm {
                            input: ProviderInput {
                                id: None,
                                name: "本地模型示例".into(),
                                base_url: "http://127.0.0.1:11434/v1".into(),
                                api_key: String::new(),
                                enabled: Some(true),
                            },
                            preset: usize::MAX,
                            show_key: false,
                        })
                    }
                    25 => {
                        self.model_form = Some(ModelForm {
                            input: ModelInput {
                                id: None,
                                provider_id: 0,
                                model: "local-model".into(),
                                display_name: Some("演示模型".into()),
                                temperature: Some(0.3),
                                max_tokens: Some(2048),
                                enabled: Some(true),
                                is_default: Some(false),
                            },
                        })
                    }
                    26 => {
                        self.question_form = Some(QuestionForm {
                            input: QuestionInput {
                                id: None,
                                title: "下列关于学习方法的说法，正确的是？".into(),
                                options: Some(
                                    "A. 主动回顾\nB. 只读不练\nC. 临时突击\nD. 忽略反馈".into(),
                                ),
                                qtype: Some("single".into()),
                                answer: "A".into(),
                            },
                        })
                    }
                    27 => {
                        self.confirm = Some(Confirm::ClearQuestions(
                            self.snapshot.as_ref().map_or(0, |s| s.stats.question_total),
                        ))
                    }
                    _ => {}
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                    if (16..28).contains(&smoke.stage) || smoke.stage == 70 {
                        Vec2::new(900.0, 600.0)
                    } else {
                        Vec2::new(1220.0, 820.0)
                    },
                ));
            }
            smoke.frame += 1;
            let wait_seconds = if (28..52).contains(&smoke.stage) {
                let icon = (smoke.stage - 28) / 3;
                if (smoke.stage - 28) % 3 == 0 {
                    0.35
                } else if icon == 1 {
                    if smoke.stage == 32 {
                        3.2
                    } else {
                        1.55
                    }
                } else if icon == 7 {
                    0.80
                } else {
                    0.55
                }
            } else if (52..60).contains(&smoke.stage) {
                if smoke.stage == 53 {
                    3.2
                } else if smoke.stage == 59 {
                    0.80
                } else {
                    0.55
                }
            } else {
                0.35
            };
            let sample_time = ctx.input(|input| input.time);
            let sample_id = egui::Id::new(("native_smoke_frame_times", smoke.stage));
            if smoke.stage == 32 || smoke.stage == 53 {
                ctx.data_mut(|data| {
                    let times = data.get_temp_mut_or_default::<Vec<f64>>(sample_id);
                    if times.last().is_none_or(|last| *last != sample_time) {
                        times.push(sample_time);
                    }
                });
            }
            if smoke.frame >= 10
                && smoke.stage_started.elapsed().as_secs_f32() >= wait_seconds
                && self.snapshot.is_some()
            {
                if (28..61).contains(&smoke.stage) {
                    let rects: Vec<[f32; 4]> = self
                        .nav_rects
                        .iter()
                        .map(|r| r.map_or([0.0; 4], |r| [r.min.x, r.min.y, r.max.x, r.max.y]))
                        .collect();
                    let times = ctx
                        .data(|data| data.get_temp::<Vec<f64>>(sample_id))
                        .unwrap_or_default();
                    let cadence = frame_cadence(&times);
                    smoke.motions.push(serde_json::json!({"stage":smoke.stage,"progress":self.nav_progress,"rects":rects,"focused":ctx.input(|i|i.focused),"elapsed":smoke.stage_started.elapsed().as_secs_f32(),"cadence":cadence}));
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                smoke.waiting = true;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(30));
        self.smoke = Some(smoke);
    }
}

fn frame_cadence(times: &[f64]) -> Option<serde_json::Value> {
    // Ignore the first four frames after the prior PNG capture and window resize.
    let mut intervals: Vec<f64> = times
        .windows(2)
        .skip(4)
        .map(|pair| (pair[1] - pair[0]) * 1000.0)
        .filter(|ms| *ms > 0.0)
        .collect();
    if intervals.len() < 10 {
        return None;
    }
    let duration_ms: f64 = intervals.iter().sum();
    intervals.sort_by(f64::total_cmp);
    Some(serde_json::json!({
        "samples": intervals.len(),
        "averageFps": intervals.len() as f64 * 1000.0 / duration_ms,
        "medianFrameMs": intervals[intervals.len() / 2],
        "p95FrameMs": intervals[(intervals.len() - 1) * 95 / 100],
        "maxFrameMs": intervals.last(),
        "over50ms": intervals.iter().filter(|ms| **ms > 50.0).count(),
    }))
}
