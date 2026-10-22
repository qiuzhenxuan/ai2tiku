mod dialogs;
mod pages;
mod paint;
mod verification;
mod widgets;

use crate::{
    db::Db,
    models::*,
    server,
    state::{AppState, SharedState},
};
use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, Vec2};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Dashboard,
    Deploy,
    Ocs,
    Providers,
    Questions,
    Records,
    Stats,
    Settings,
}

impl Page {
    const ALL: [Self; 8] = [
        Self::Dashboard,
        Self::Deploy,
        Self::Ocs,
        Self::Providers,
        Self::Questions,
        Self::Records,
        Self::Stats,
        Self::Settings,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "概览",
            Self::Deploy => "部署向导",
            Self::Ocs => "OCS 接入",
            Self::Providers => "供应商与模型",
            Self::Questions => "本地题库",
            Self::Records => "答题记录",
            Self::Stats => "用量统计",
            Self::Settings => "设置",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Dashboard => "题库、服务与最近的学习记录",
            Self::Deploy => "从浏览器安装到实际搜题，逐步完成接入",
            Self::Ocs => "复制完整题库配置，在 OCS 中验证连接",
            Self::Providers => "连接模型服务，管理可用模型与默认选择",
            Self::Questions => "积累经过校验的答案，让每一次查找更快",
            Self::Records => "查看请求来源、模型、耗时与答案",
            Self::Stats => "了解缓存命中与模型用量",
            Self::Settings => "服务、缓存与界面偏好",
        }
    }
}

#[derive(Clone, Default)]
struct Query {
    keyword: String,
    source: String,
    questions_page: i64,
    records_page: i64,
}

#[derive(Clone)]
struct Snapshot {
    deployment: DeploymentStatus,
    stats: Stats,
    providers: Vec<Provider>,
    models: Vec<AiModel>,
    questions: crate::models::Page<Question>,
    records: crate::models::Page<SearchRecord>,
    recent: Vec<SearchRecord>,
}

impl Snapshot {
    fn load(state: &SharedState, q: &Query) -> Result<Self, String> {
        Ok(Self {
            deployment: server::deployment_status(state)?,
            stats: state.db.stats()?,
            providers: state.db.list_providers()?,
            models: state.db.list_models()?,
            questions: state.db.list_questions(
                Some(q.keyword.clone()),
                Some(q.source.clone()),
                q.questions_page.max(1),
                20,
            )?,
            records: state.db.list_records(q.records_page.max(1), 20)?,
            recent: state.db.list_records(1, 6)?.items,
        })
    }
}

enum Event {
    Snapshot(u64, Result<Snapshot, String>),
    Notice(&'static str, Result<String, String>),
    Release(Result<OcsRelease, String>),
    Search(SearchOutcome),
    ProviderSaved(Result<(i64, String), String>),
    ModelSaved(Result<String, String>),
    QuestionSaved(Result<String, String>),
    Copied(Result<String, String>),
}

#[derive(Clone)]
enum Confirm {
    DeleteProvider(Provider),
    DeleteModel(AiModel),
    DeleteQuestion(Question),
    ClearQuestions(i64),
    ClearRecords(i64),
}

struct ProviderForm {
    input: ProviderInput,
    preset: usize,
    show_key: bool,
}
struct ModelForm {
    input: ModelInput,
}
struct QuestionForm {
    input: QuestionInput,
}
struct Notification {
    text: String,
    error: bool,
    created: Instant,
}

pub(super) struct NativeApp {
    state: SharedState,
    runtime: tokio::runtime::Runtime,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    page: Page,
    snapshot: Option<Snapshot>,
    query: Query,
    revision: u64,
    refreshing: bool,
    refresh_queued: bool,
    refresh_error: Option<String>,
    last_refresh: Instant,
    pending: HashSet<&'static str>,
    settings: Settings,
    dark: bool,
    reduce_motion: bool,
    sidebar_expanded: bool,
    sidebar_toggle_rect: Option<egui::Rect>,
    notification: Option<Notification>,
    provider_form: Option<ProviderForm>,
    model_form: Option<ModelForm>,
    question_form: Option<QuestionForm>,
    confirm: Option<Confirm>,
    search_title: String,
    search_options: String,
    search_type: String,
    search_result: Option<SearchOutcome>,
    release: Option<OcsRelease>,
    release_error: Option<String>,
    browser: BrowserInfo,
    smoke: Option<Smoke>,
    logo: egui::TextureHandle,
    logo_dark: egui::TextureHandle,
    first_frame: bool,
    nav_rects: [Option<egui::Rect>; 8],
    nav_ids: [Option<egui::Id>; 8],
    nav_progress: [f32; 8],
}

struct Smoke {
    dir: PathBuf,
    frame: usize,
    stage: usize,
    waiting: bool,
    stage_started: Instant,
    motions: Vec<serde_json::Value>,
}

fn data_path(override_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    let dir = match override_dir {
        Some(dir) => dir,
        None => dirs::data_dir()
            .ok_or("找不到应用数据目录")?
            .join("com.ai2tiku.app"),
    };
    Ok(dir.join("ai2tiku.db"))
}

pub fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let smoke = args
        .windows(2)
        .find(|a| a[0] == "--smoke-test")
        .map(|a| PathBuf::from(&a[1]));
    let override_dir = args
        .windows(2)
        .find(|a| a[0] == "--data-dir")
        .map(|a| PathBuf::from(&a[1]));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let db = Db::open(&data_path(smoke.clone().or(override_dir))?)?;
    let state = Arc::new(AppState::new(db));
    let mut startup_error = None;
    if smoke.is_none() {
        let port = state.db.settings()?.port;
        if let Err(e) = server::start(&state, port) {
            startup_error = Some(format!("本地服务未启动：{e}"));
        }
    } else {
        seed_preview(&state)?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);
        server::start(&state, port)?;
    }
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../icons/icon.png"))
        .map_err(|e| e.to_string())?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ai2tiku · AI 题库")
            .with_inner_size([1220.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_clamp_size_to_monitor_size(true)
            .with_decorations(false)
            .with_icon(icon),
        centered: true,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let cleanup = state.clone();
    let result = eframe::run_native(
        "ai2tiku",
        options,
        Box::new(move |cc| {
            Ok(Box::new(NativeApp::new(
                &cc.egui_ctx,
                state,
                runtime,
                startup_error,
                smoke,
            )?))
        }),
    )
    .map_err(|e| e.to_string());
    server::stop(&cleanup);
    result
}

impl NativeApp {
    fn new(
        ctx: &egui::Context,
        state: SharedState,
        runtime: tokio::runtime::Runtime,
        startup_error: Option<String>,
        smoke: Option<PathBuf>,
    ) -> Result<Self, String> {
        install_fonts(ctx);
        let (tx, rx) = mpsc::channel();
        let settings = state.db.settings()?;
        let dark = state.db.ui_pref("theme")?.as_deref() == Some("dark");
        let reduce_motion = state
            .db
            .ui_pref("native_reduce_motion")?
            .map_or_else(system_reduce_motion, |v| v == "true");
        let browser = crate::browser::info();
        let mut decoded = image::load_from_memory(include_bytes!(
            "../../../src/assets/brand/infinity-brush.png"
        ))
        .map_err(|e| e.to_string())?
        .to_rgba8();
        for pixel in decoded.pixels_mut() {
            let gray = (0.2126 * pixel.0[0] as f32
                + 0.7152 * pixel.0[1] as f32
                + 0.0722 * pixel.0[2] as f32)
                .round() as u8;
            pixel.0[0] = gray;
            pixel.0[1] = gray;
            pixel.0[2] = gray;
        }
        let dimensions = [decoded.width() as usize, decoded.height() as usize];
        let logo = ctx.load_texture(
            "infinity-brush",
            egui::ColorImage::from_rgba_unmultiplied(dimensions, decoded.as_raw()),
            egui::TextureOptions::LINEAR,
        );
        let mut inverse = decoded.clone();
        for pixel in inverse.pixels_mut() {
            pixel.0[0] = 255 - pixel.0[0];
            pixel.0[1] = 255 - pixel.0[1];
            pixel.0[2] = 255 - pixel.0[2];
        }
        let logo_dark = ctx.load_texture(
            "infinity-brush-dark",
            egui::ColorImage::from_rgba_unmultiplied(dimensions, inverse.as_raw()),
            egui::TextureOptions::LINEAR,
        );
        let mut app = Self {
            state,
            runtime,
            tx,
            rx,
            page: Page::Dashboard,
            snapshot: None,
            query: Query {
                questions_page: 1,
                records_page: 1,
                ..Default::default()
            },
            revision: 0,
            refreshing: false,
            refresh_queued: false,
            refresh_error: None,
            last_refresh: Instant::now(),
            pending: HashSet::new(),
            settings,
            dark,
            reduce_motion,
            sidebar_expanded: false,
            sidebar_toggle_rect: None,
            notification: startup_error.map(|text| Notification {
                text,
                error: true,
                created: Instant::now(),
            }),
            provider_form: None,
            model_form: None,
            question_form: None,
            confirm: None,
            search_title: String::new(),
            search_options: String::new(),
            search_type: "single".into(),
            search_result: None,
            release: None,
            release_error: None,
            browser,
            smoke: smoke.map(|dir| Smoke {
                dir,
                frame: 0,
                stage: 0,
                waiting: false,
                stage_started: Instant::now(),
                motions: Vec::new(),
            }),
            logo,
            logo_dark,
            first_frame: true,
            nav_rects: [None; 8],
            nav_ids: [None; 8],
            nav_progress: [0.0; 8],
        };
        widgets::theme(ctx, app.dark);
        app.refresh(ctx);
        Ok(app)
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        self.revision += 1;
        if self.refreshing {
            self.refresh_queued = true;
            return;
        }
        self.refreshing = true;
        let state = self.state.clone();
        let q = self.query.clone();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let revision = self.revision;
        self.runtime.spawn_blocking(move || {
            let _ = tx.send(Event::Snapshot(revision, Snapshot::load(&state, &q)));
            ctx.request_repaint();
        });
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notification = Some(Notification {
            text: text.into(),
            error,
            created: Instant::now(),
        });
    }

    fn job<F>(&mut self, ctx: &egui::Context, key: &'static str, work: F)
    where
        F: std::future::Future<Output = Event> + Send + 'static,
    {
        if !self.pending.insert(key) {
            return;
        }
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            let event = work.await;
            let _ = tx.send(event);
            ctx.request_repaint();
        });
    }

    fn process_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Snapshot(revision, result) => {
                    self.refreshing = false;
                    self.last_refresh = Instant::now();
                    if revision == self.revision {
                        match result {
                            Ok(snapshot) => {
                                self.snapshot = Some(snapshot);
                                self.refresh_error = None;
                            }
                            Err(e) => {
                                self.snapshot = None;
                                self.refresh_error = Some(e);
                            }
                        }
                    }
                    if self.refresh_queued {
                        self.refresh_queued = false;
                        self.refresh(ctx);
                    }
                }
                Event::Notice(key, result) => {
                    self.pending.remove(key);
                    match result {
                        Ok(s) => {
                            self.notify(s, false);
                            self.refresh(ctx);
                        }
                        Err(e) => self.notify(e, true),
                    }
                }
                Event::Release(result) => {
                    self.pending.remove("release");
                    match result {
                        Ok(r) => {
                            self.release = Some(r);
                            self.release_error = None;
                        }
                        Err(e) => {
                            self.release = None;
                            self.release_error = Some(e);
                        }
                    }
                }
                Event::Search(result) => {
                    self.pending.remove("search");
                    self.search_result = Some(result);
                    self.refresh(ctx);
                }
                Event::ProviderSaved(result) => {
                    self.pending.remove("save_provider");
                    match result {
                        Ok((id, name)) => {
                            self.provider_form = None;
                            self.notify(format!("{name} 已保存，正在读取模型…"), false);
                            self.sync_models(ctx, id);
                            self.refresh(ctx);
                        }
                        Err(e) => self.notify(e, true),
                    }
                }
                Event::ModelSaved(result) => {
                    self.pending.remove("save_model");
                    match result {
                        Ok(text) => {
                            self.model_form = None;
                            self.notify(text, false);
                            self.refresh(ctx);
                        }
                        Err(e) => self.notify(e, true),
                    }
                }
                Event::QuestionSaved(result) => {
                    self.pending.remove("save_question");
                    match result {
                        Ok(text) => {
                            self.question_form = None;
                            self.notify(text, false);
                            self.refresh(ctx);
                        }
                        Err(e) => self.notify(e, true),
                    }
                }
                Event::Copied(result) => {
                    self.pending.remove("copy");
                    match result {
                        Ok(_) => self.notify("题库 JSON 已复制", false),
                        Err(e) => self.notify(e, true),
                    }
                }
            }
        }
    }

    fn sync_models(&mut self, ctx: &egui::Context, id: i64) {
        let state = self.state.clone();
        self.job(ctx, "sync", async move {
            Event::Notice(
                "sync",
                crate::services::sync_models(&state, id).await.map(|s| {
                    format!(
                        "{}：获取 {} 个模型，新增 {} 个",
                        s.provider, s.total, s.added
                    )
                }),
            )
        });
    }

    fn copy_config(&mut self, ctx: &egui::Context) {
        let state = self.state.clone();
        self.job(ctx, "copy", async move {
            Event::Copied(
                tokio::task::spawn_blocking(move || {
                    let snapshot = server::deployment_status(&state)?;
                    if !snapshot.server.running {
                        return Err("请先启动本地服务，再复制配置".into());
                    }
                    if snapshot.server.port == 0 {
                        return Err("服务端口无效，请检查设置".into());
                    }
                    let mut clipboard =
                        arboard::Clipboard::new().map_err(|e| format!("无法访问剪贴板：{e}"))?;
                    clipboard
                        .set_text(snapshot.config)
                        .map_err(|e| format!("复制失败：{e}"))?;
                    Ok(String::new())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string())),
            )
        });
    }

    fn open(&mut self, ctx: &egui::Context, url: impl Into<String>) {
        let url = url.into();
        self.job(ctx, "browser", async move {
            Event::Notice(
                "browser",
                tokio::task::spawn_blocking(move || {
                    crate::browser::open_url(&url).map(|_| "已在 Edge 中打开".into())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string())),
            )
        });
    }

    fn toggle_service(&mut self, ctx: &egui::Context) {
        let state = self.state.clone();
        let running = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.deployment.server.running);
        self.job(ctx, "service", async move {
            Event::Notice(
                "service",
                tokio::task::spawn_blocking(move || {
                    if running {
                        server::stop(&state);
                        Ok("服务已停止".into())
                    } else {
                        server::start(&state, state.db.settings()?.port)
                            .map(|_| "服务已启动".into())
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string())),
            )
        });
    }

    fn load_release(&mut self, ctx: &egui::Context) {
        let state = self.state.clone();
        self.job(ctx, "release", async move {
            Event::Release(crate::services::latest_release(&state).await)
        });
    }
}

impl eframe::App for NativeApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.verification_input(ctx, input);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            if self.smoke.is_none() {
                if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(initial_size(monitor)));
                }
            }
        }
        self.process_events(ctx);
        if !self.refreshing && self.last_refresh.elapsed() > Duration::from_secs(5) {
            self.refresh(ctx);
        }
        self.shell(ctx);
        self.dialogs(ctx);
        widgets::interactive_cursor(ctx);
        self.smoke_tick(ctx);
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for path in [
        "C:/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/PingFang.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("chinese".into(), FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese".into());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("chinese".into());
            break;
        }
    }
    let mut heading = fonts.families[&FontFamily::Proportional].clone();
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/simkai.ttf") {
        fonts
            .font_data
            .insert("ink".into(), FontData::from_owned(bytes).into());
        heading.insert(0, "ink".into());
    }
    fonts
        .families
        .insert(FontFamily::Name("ink".into()), heading);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(26.0, FontFamily::Name("ink".into())),
        );
        s.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        s.spacing.item_spacing = Vec2::new(10.0, 10.0);
        s.spacing.button_padding = Vec2::new(14.0, 8.0);
        s.spacing.interact_size.y = widgets::CONTROL_HEIGHT;
    });
}

fn initial_size(monitor: Vec2) -> Vec2 {
    Vec2::new(
        1220.0_f32.min((monitor.x * 0.92).max(900.0)),
        820.0_f32.min((monitor.y * 0.90).max(600.0)),
    )
}

fn system_reduce_motion() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn SystemParametersInfoW(
                action: u32,
                param: u32,
                value: *mut std::ffi::c_void,
                flags: u32,
            ) -> i32;
        }
        let mut enabled = 1i32;
        // SPI_GETCLIENTAREAANIMATION, a read-only system preference query.
        return unsafe {
            SystemParametersInfoW(4162, 0, (&mut enabled as *mut i32).cast(), 0) != 0
                && enabled == 0
        };
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn seed_preview(state: &SharedState) -> Result<(), String> {
    if state.db.list_questions(None, None, 1, 1)?.total == 0 {
        state.db.save_question(&QuestionInput {
            id: None,
            title: "下列关于学习方法的说法，正确的是？".into(),
            options: Some("A. 主动回顾\nB. 只读不练\nC. 临时突击\nD. 忽略反馈".into()),
            qtype: Some("single".into()),
            answer: "A".into(),
        })?;
        state.db.add_record(
            "下列关于学习方法的说法，正确的是？",
            "A",
            "cache",
            "",
            "",
            0,
            0,
            12,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (egui::Context, NativeApp) {
        let path = std::env::temp_dir().join(format!(
            "ai2tiku-native-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state = Arc::new(AppState::new(Db::open(&path).unwrap()));
        seed_preview(&state).unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let ctx = egui::Context::default();
        let mut app = NativeApp::new(&ctx, state, runtime, None, None).unwrap();
        app.snapshot = Some(Snapshot::load(&app.state, &app.query).unwrap());
        (ctx, app)
    }

    #[test]
    fn all_native_pages_render_in_both_themes_and_window_sizes() {
        let (ctx, mut app) = app();
        for dark in [false, true] {
            app.dark = dark;
            widgets::theme(&ctx, dark);
            for expanded in [false, true] {
                app.sidebar_expanded = expanded;
                for size in [Vec2::new(1220.0, 820.0), Vec2::new(900.0, 600.0)] {
                    for page in Page::ALL {
                        app.page = page;
                        for _ in 0..2 {
                            let mut used = egui::Rect::NOTHING;
                            let output = ctx.run(
                                egui::RawInput {
                                    screen_rect: Some(egui::Rect::from_min_size(
                                        egui::Pos2::ZERO,
                                        size,
                                    )),
                                    ..Default::default()
                                },
                                |ctx| {
                                    app.shell(ctx);
                                    used = ctx.used_rect();
                                },
                            );
                            assert!(!output.shapes.is_empty());
                            assert!(used.right() <= size.x + 1.0, "{page:?} {size:?}: {used:?}");
                            let visuals = ctx.style().visuals.clone();
                            let selected = visuals.selection.bg_fill;
                            assert_eq!(selected.r(), selected.g());
                            assert_eq!(selected.g(), selected.b());
                            assert_eq!(visuals.dark_mode, dark);
                            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
                            assert!(!primitives.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sidebar_defaults_to_icons_and_click_toggle_preserves_navigation_alignment() {
        fn frame(ctx: &egui::Context, app: &mut NativeApp, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(900.0, 600.0),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| app.shell(ctx),
            );
        }
        fn click(ctx: &egui::Context, app: &mut NativeApp, at: egui::Pos2) {
            frame(ctx, app, vec![egui::Event::PointerMoved(at)]);
            for pressed in [true, false] {
                frame(
                    ctx,
                    app,
                    vec![egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
            frame(ctx, app, vec![]);
        }
        let (ctx, mut app) = app();
        assert!(!app.sidebar_expanded);
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let toggle = app.sidebar_toggle_rect.unwrap();
        let ids = app.nav_ids;
        let collapsed = app.nav_rects;
        click(&ctx, &mut app, toggle.left_center() + Vec2::new(24.0, 0.0));
        assert!(app.sidebar_expanded);
        assert_eq!(app.nav_ids, ids, "Widget identities must survive expansion");
        assert_eq!(app.sidebar_toggle_rect.unwrap().min, toggle.min);
        for index in 0..8 {
            let before = collapsed[index].unwrap();
            let after = app.nav_rects[index].unwrap();
            assert_eq!(
                before.left_center(),
                after.left_center(),
                "Icon origin shifted at {index}"
            );
            assert!((after.width() - before.width() - 128.0).abs() < 0.1);
        }
        let settings = app.nav_rects[7].unwrap();
        click(&ctx, &mut app, settings.center());
        assert_eq!(app.page, Page::Settings);
        assert!(
            app.sidebar_expanded,
            "Navigation must not unexpectedly collapse the rail"
        );
        click(&ctx, &mut app, toggle.left_center() + Vec2::new(24.0, 0.0));
        assert!(!app.sidebar_expanded);
        assert_eq!(app.nav_ids, ids);
        assert_eq!(app.nav_rects, collapsed);
        assert_eq!(app.page, Page::Settings);
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("sidebar_toggle")));
        frame(&ctx, &mut app, vec![]);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            app.sidebar_expanded,
            "Toggle must also work from the keyboard"
        );
    }

    #[test]
    fn failed_save_preserves_draft_and_stale_snapshot_is_ignored() {
        let (ctx, mut app) = app();
        app.question_form = Some(QuestionForm {
            input: QuestionInput {
                id: None,
                title: "保留的草稿".into(),
                options: None,
                qtype: Some("completion".into()),
                answer: "答案".into(),
            },
        });
        app.pending.insert("save_question");
        app.tx
            .send(Event::QuestionSaved(Err("写入失败".into())))
            .unwrap();
        app.process_events(&ctx);
        assert_eq!(
            app.question_form.as_ref().unwrap().input.title,
            "保留的草稿"
        );
        assert!(!app.pending.contains("save_question"));
        let expected = app.snapshot.as_ref().unwrap().questions.total;
        app.revision = 1000;
        app.tx
            .send(Event::Snapshot(999, Err("过时错误".into())))
            .unwrap();
        app.process_events(&ctx);
        assert_eq!(app.snapshot.as_ref().unwrap().questions.total, expected);
        assert!(app.refresh_error.is_none());
    }

    #[test]
    fn native_cache_search_and_settings_use_the_shared_business_layer() {
        let (_, app) = app();
        let query = SearchQuery {
            title: "下列关于学习方法的说法，正确的是？".into(),
            qtype: "single".into(),
            options: "A. 主动回顾\nB. 只读不练\nC. 临时突击\nD. 忽略反馈".into(),
            ..Default::default()
        };
        let answer = app
            .runtime
            .block_on(crate::search::answer(&app.state, &query));
        assert_eq!(answer.code, 1);
        assert_eq!(answer.source, "cache");
        assert_eq!(answer.answer.as_deref(), Some("A"));
        assert_eq!(answer.tokens, 0);
        let mut settings = app.state.db.settings().unwrap();
        settings.ai_fallback = false;
        server::apply_settings(&app.state, settings).unwrap();
        let miss = app.runtime.block_on(crate::search::answer(
            &app.state,
            &SearchQuery {
                title: "未知题目".into(),
                qtype: "completion".into(),
                ..Default::default()
            },
        ));
        assert_eq!(miss.code, 0);
        assert!(miss.msg.unwrap().contains("AI 兜底已关闭"));
    }
    #[test]
    fn native_data_path_keeps_existing_database_location() {
        let dir = PathBuf::from("example");
        assert_eq!(
            data_path(Some(dir.clone())).unwrap(),
            dir.join("ai2tiku.db")
        );
        assert_eq!(
            data_path(None).unwrap(),
            dirs::data_dir().unwrap().join("com.ai2tiku.app/ai2tiku.db")
        );
    }

    #[test]
    fn navigation_keyboard_cycles_activates_and_dismisses_focus_help() {
        fn frame(ctx: &egui::Context, app: &mut NativeApp, key: Option<(egui::Key, bool)>) {
            let events = key.map_or_else(Vec::new, |(key, pressed)| {
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]
            });
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1220.0, 820.0),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| app.shell(ctx),
            );
        }
        fn press(ctx: &egui::Context, app: &mut NativeApp, key: egui::Key) {
            frame(ctx, app, Some((key, true)));
            frame(ctx, app, Some((key, false)));
        }
        let (ctx, mut app) = app();
        frame(&ctx, &mut app, None);
        frame(&ctx, &mut app, None);
        ctx.memory_mut(|memory| memory.request_focus(app.nav_ids[0].unwrap()));
        frame(&ctx, &mut app, None);
        for (key, index) in [
            (egui::Key::ArrowUp, 7),
            (egui::Key::ArrowDown, 0),
            (egui::Key::End, 7),
            (egui::Key::Home, 0),
            (egui::Key::ArrowDown, 1),
        ] {
            press(&ctx, &mut app, key);
            assert_eq!(ctx.memory(|m| m.focused()), app.nav_ids[index], "{key:?}");
            assert_eq!(
                app.page,
                Page::Dashboard,
                "Focus movement must not activate a page"
            );
        }
        press(&ctx, &mut app, egui::Key::Enter);
        assert_eq!(app.page, Page::Deploy);
        press(&ctx, &mut app, egui::Key::ArrowDown);
        press(&ctx, &mut app, egui::Key::Space);
        assert_eq!(app.page, Page::Ocs);
        let id = app.nav_ids[2].unwrap();
        let dismissed_id = id.with("navigation_tip_dismissed");
        assert_eq!(
            ctx.data(|data| data.get_temp::<bool>(dismissed_id)),
            Some(false)
        );
        press(&ctx, &mut app, egui::Key::Escape);
        frame(&ctx, &mut app, None);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(id),
            "Escape only closes help"
        );
        assert_eq!(
            ctx.data(|data| data.get_temp::<bool>(dismissed_id)),
            Some(true)
        );
        press(&ctx, &mut app, egui::Key::ArrowDown);
        press(&ctx, &mut app, egui::Key::ArrowUp);
        assert_eq!(
            ctx.data(|data| data.get_temp::<bool>(dismissed_id)),
            Some(false)
        );
    }
}
