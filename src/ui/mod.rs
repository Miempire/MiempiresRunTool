//! 应用主界面：顶部栏 + 两个标签（跑步 / 运动记录）+ 各种弹窗。
//!
//! 设计原则：极简、双层信息（主界面只留入口，配置全部收进弹窗）。

pub mod auth;
pub mod fonts;
pub mod log;
pub mod map;
pub mod mobile;
pub mod records_page;
pub mod run_page;
pub mod theme;
pub mod update_dialog;

use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;
use egui::{Align, Color32, Layout, RichText};

use crate::api::model::Session;
use crate::api::records::RecordRow;
use crate::config;
use crate::log_upload;
use crate::soft::client::ApiError;
use crate::soft::model::{LicenseInfo, LoginResult, ReleaseInfo, RoadInfo, SoftAccount};
use crate::soft::session::SoftSession;

use auth::{AuthUi, OfficialUi};
use records_page::RecordsUi;
use run_page::{RoadsUi, RunUi};
use theme::ThemeKind;
use update_dialog::UpdateUi;

/// 后台任务回传的消息。
pub enum Msg {
    SoftLogin(Result<LoginResult, ApiError>),
    SoftRegister(Result<String, ApiError>),
    Account(SoftAccount, LicenseInfo),
    License(Result<LicenseInfo, ApiError>),
    Roads(Result<Vec<RoadInfo>, ApiError>),
    RoadPrepared {
        road: RoadInfo,
        preview: Result<Box<crate::road_editor::Preview>, String>,
        network: Option<std::sync::Arc<route_planner::RoadGraph>>,
        path: Option<std::path::PathBuf>,
    },
    RoadProgress(u64, u64),
    RoadUse(Result<(bool, i64), ApiError>),
    RunLog(String),
    RunDone(Result<run_page::RunOutcome, String>),
    /// 必经点/检查点拉取结果（BD09 坐标）。
    Checkpoints(Result<Vec<(f64, f64)>, String>),
    Records(Result<Vec<RecordRow>, String>),
    OfficialLogin(Result<Session, String>),
    Update(Result<Option<ReleaseInfo>, ApiError>),
    UpdateProgress(u64, u64),
    UpdateDownloaded(Result<std::path::PathBuf, ApiError>),
    LogsUploaded(Result<usize, ApiError>),
    Toast(String),
}

pub struct App {
    pub ctx: egui::Context,
    pub palette: theme::Palette,
    pub theme: ThemeKind,
    pub settings: config::Settings,

    pub session: Option<SoftSession>,
    pub account: Option<SoftAccount>,
    pub auth: AuthUi,
    pub official: OfficialUi,
    pub roads: RoadsUi,
    pub run: RunUi,
    pub records: RecordsUi,
    pub update: UpdateUi,
    pub log: log::LogStore,

    pub tab: usize,
    pub booting: bool,
    pub toast: Option<(String, f64)>,

    pub account_open: bool,
    pub server_open: bool,
    pub server_input: String,
    pub server_error: Option<String>,
    pub about_open: bool,

    pub last_license_at: f64,
    pub last_upload_at: f64,
    pub last_update_at: f64,
    pub last_use_flush_at: f64,
    pub update_checked: bool,
    pub uploading_logs: bool,
    pub device_requested: bool,

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

pub fn now_secs(ctx: &egui::Context) -> f64 {
    ctx.input(|input| input.time)
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = config::load();
        let theme = ThemeKind::parse(&settings.theme);
        let palette = theme.palette();
        fonts::install(&cc.egui_ctx);
        palette.apply(&cc.egui_ctx);

        let (tx, rx) = mpsc::channel();
        let session = SoftSession::load();
        let booting = session.is_some();
        let mut app = Self {
            ctx: cc.egui_ctx.clone(),
            palette,
            theme,
            settings: settings.clone(),
            session,
            account: None,
            auth: AuthUi::default(),
            official: OfficialUi {
                session: Some(crate::api::model::load_session())
                    .filter(|s| !s.token.trim().is_empty()),
                ..Default::default()
            },
            roads: RoadsUi::default(),
            run: RunUi::new(&settings),
            records: RecordsUi::default(),
            update: UpdateUi::default(),
            log: log::LogStore::default(),
            tab: 0,
            booting,
            toast: None,
            account_open: false,
            server_open: false,
            server_input: settings.server_base.clone(),
            server_error: None,
            about_open: false,
            last_license_at: 0.0,
            last_upload_at: 0.0,
            last_update_at: 0.0,
            last_use_flush_at: 0.0,
            update_checked: false,
            uploading_logs: false,
            device_requested: false,
            tx,
            rx,
        };

        if app.session.is_some() {
            app.refresh_license();
        }

        #[cfg(target_os = "android")]
        crate::android::request_device_info(false);

        log_upload::info(&format!(
            "启动 Miempire's RUN Tool {}（服务器 {}）",
            crate::platform::version_name(),
            config::base_url()
        ));
        app
    }

    pub fn spawn<F>(&self, job: F)
    where
        F: FnOnce(Sender<Msg>) + Send + 'static,
    {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            job(tx);
            ctx.request_repaint();
        });
    }

    pub fn token(&self) -> Option<String> {
        self.session
            .as_ref()
            .map(|session| session.token.clone())
            .filter(|token| !token.is_empty())
    }

    pub fn can_use(&self) -> bool {
        !self.booting && self.session.as_ref().map(|s| s.is_active()).unwrap_or(false)
    }

    pub fn toast(&mut self, message: impl Into<String>) {
        let until = now_secs(&self.ctx) + 2.8;
        self.toast = Some((message.into(), until));
    }

    // ── 后台任务 ────────────────────────────────────────────

    pub fn refresh_license(&mut self) {
        let Some(session) = self.session.clone() else {
            self.booting = false;
            return;
        };
        self.booting = self.session.as_ref().map(|s| s.status.is_empty()).unwrap_or(false);
        self.spawn(move |tx| {
            let device_id = crate::soft::session::device_id();
            let result = crate::soft::client::license_check(&session.token, &device_id);
            let _ = tx.send(Msg::License(result));
            if let Ok((account, license)) = crate::soft::client::me(&session.token, &device_id) {
                let _ = tx.send(Msg::Account(account, license));
            }
        });
    }

    pub fn login(&mut self, username: String, password: String) {
        self.auth.busy = true;
        self.auth.error = None;
        self.spawn(move |tx| {
            let device_id = crate::soft::session::device_id();
            let device_name = crate::soft::session::device_name();
            let result = crate::soft::client::login(&username, &password, &device_id, &device_name);
            let _ = tx.send(Msg::SoftLogin(result));
        });
    }

    pub fn register(&mut self, username: String, password: String, nickname: String) {
        self.auth.busy = true;
        self.auth.error = None;
        self.spawn(move |tx| {
            let device_id = crate::soft::session::device_id();
            let device_name = crate::soft::session::device_name();
            let result = crate::soft::client::register(
                &username,
                &password,
                &nickname,
                &device_id,
                &device_name,
                &crate::platform::version_name(),
            );
            let _ = tx.send(Msg::SoftRegister(result));
        });
    }

    pub fn logout(&mut self) {
        if let Some(session) = self.session.take() {
            let device_id = crate::soft::session::device_id();
            self.spawn(move |tx| {
                crate::soft::client::logout(&session.token, &device_id);
                let _ = tx.send(Msg::Toast("已退出登录".into()));
            });
        }
        SoftSession::clear();
        self.account = None;
        self.booting = false;
        self.update_checked = false;
        self.roads = RoadsUi::default();
        self.run.reset();
        self.records = RecordsUi::default();
    }

    /// 退出官方账号：清除本地加密会话与运动记录缓存。
    pub fn official_logout(&mut self) {
        self.official.session = None;
        self.official.username.clear();
        self.official.password.clear();
        self.official.error = None;
        self.records = RecordsUi::default();
        crate::api::model::clear_session();
        crate::api::model::clear_points_cache();
        self.log.push("已退出官方账号");
    }

    pub fn upload_logs(&mut self) {
        if self.uploading_logs || log_upload::pending_count() == 0 {
            return;
        }
        let Some(token) = self.token() else { return };
        self.uploading_logs = true;
        self.spawn(move |tx| {
            let device_id = crate::soft::session::device_id();
            let batch = log_upload::pending_json(200);
            let count = batch.len();
            let result = crate::soft::client::logs_batch(
                &token,
                &device_id,
                &crate::platform::version_name(),
                &batch,
            )
            .map(|_| {
                log_upload::ack(count);
                count
            });
            let _ = tx.send(Msg::LogsUploaded(result));
        });
    }

    pub fn check_update(&mut self, manual: bool) {
        let Some(token) = self.token() else { return };
        self.update.busy = true;
        self.update.error = None;
        self.spawn(move |tx| {
            let device_id = crate::soft::session::device_id();
            let result = crate::soft::update::check(&token, &device_id);
            if manual {
                if let Ok(None) = &result {
                    let _ = tx.send(Msg::Toast("已是最新版本".into()));
                }
            }
            let _ = tx.send(Msg::Update(result));
        });
    }

    // ── 消息处理 ────────────────────────────────────────────

    fn pump(&mut self) {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        for event in events {
            self.handle(event);
        }
    }

    fn handle(&mut self, event: Msg) {
        match event {
            Msg::SoftLogin(Ok(login)) => {
                let session = SoftSession::from_login(&login);
                let _ = session.save();
                self.session = Some(session);
                self.account = Some(login.account);
                self.auth.busy = false;
                self.auth.error = None;
                self.auth.open = false;
                self.booting = false;
                self.log
                    .push(&format!("登录成功：{}", self.session.as_ref().map(|s| s.display_name()).unwrap_or_default()));
                self.toast("登录成功");
                log_upload::info("软件账号登录成功");
                // 刷新权威授权状态（登录响应已含，但再确认一次并同步设备）。
                self.refresh_license();
            }
            Msg::SoftLogin(Err(error)) => {
                self.auth.busy = false;
                self.auth.error = Some(error.message);
            }
            Msg::Account(account, license) => {
                self.account = Some(account);
                if let Some(session) = self.session.as_mut() {
                    session.apply_license(&license);
                    let _ = session.save();
                }
            }
            Msg::SoftRegister(result) => {
                self.auth.busy = false;
                match result {
                    Ok(message) => {
                        self.auth.mode = auth::AuthMode::Login;
                        self.auth.error = None;
                        self.auth.notice = Some(message);
                    }
                    Err(error) => self.auth.error = Some(error.message),
                }
            }
            Msg::License(result) => {
                self.booting = false;
                match result {
                    Ok(license) => {
                        if let Some(session) = self.session.as_mut() {
                            let was_active = session.is_active();
                            session.apply_license(&license);
                            let _ = session.save();
                            if !was_active && session.is_active() {
                                self.toast("授权已恢复");
                            }
                        }
                    }
                    Err(error) => {
                        if error.is_license() {
                            if let Some(session) = self.session.as_mut() {
                                session.status = match error.code.as_str() {
                                    "license_expired" => "expired",
                                    "license_inactive" => "pending",
                                    "account_disabled" => "disabled",
                                    _ => "expired",
                                }
                                .into();
                                session.days_left = 0;
                                let _ = session.save();
                            }
                            self.run.reset();
                        } else if error.is_unauthorized() {
                            self.session = None;
                            SoftSession::clear();
                            self.toast("登录已失效，请重新登录");
                        } else {
                            self.toast("授权校验失败，请检查网络");
                        }
                    }
                }
            }
            Msg::Roads(result) => {
                self.roads.loading = false;
                match result {
                    Ok(list) => {
                        self.roads.error = None;
                        self.roads.list = list;
                    }
                    Err(error) => {
                        self.roads.error = Some(error.message);
                    }
                }
            }
            Msg::RoadPrepared {
                road,
                preview,
                network,
                path,
            } => {
                self.run.preparing = false;
                match (preview, network, path) {
                    (Ok(preview), Some(network), Some(path)) => {
                        self.run.selected = Some(road.clone());
                        self.run.preview = Some(run_page::PreviewState {
                            road,
                            network,
                            path,
                            preview: *preview,
                            selected_vertex: None,
                            validation: Ok(()),
                            map: map::MapState::default(),
                            last_map_key: String::new(),
                            candidate_page: 0,
                            checkpoints: Vec::new(),
                            checkpoints_loading: false,
                            checkpoints_error: None,
                        });
                        self.run.step = run_page::RunStep::Preview;
                        self.run.error = None;
                    }
                    (Err(message), _, _) => {
                        self.run.error = Some(message);
                        self.run.step = run_page::RunStep::PickRoad;
                    }
                    _ => {
                        self.run.error = Some("路网预览失败：数据不完整".into());
                        self.run.step = run_page::RunStep::PickRoad;
                    }
                }
            }
            Msg::RoadProgress(done, total) => {
                self.run.download = if total == 0 {
                    0.0
                } else {
                    (done as f32 / total as f32).clamp(0.0, 1.0)
                };
            }
            Msg::RoadUse(result) => match result {
                Ok((deduped, count)) => {
                    if let Some(road) = self.run.selected.as_mut() {
                        road.usage_count = count;
                    }
                    self.log.push(&format!(
                        "路网使用已记录{}（累计 {count} 次）",
                        if deduped { "，本次去重" } else { "" }
                    ));
                }
                Err(error) => {
                    self.log.push(&format!("路网使用上报失败：{error}（已排队重试）"));
                }
            },
            Msg::RunLog(line) => {
                self.log.push(&line);
            }
            Msg::RunDone(result) => {
                self.run.running = false;
                match result {
                    Ok(outcome) => {
                        self.run.step = run_page::RunStep::Done;
                        self.run.result = Some(outcome.clone());
                        self.log.push(&format!(
                            "完成：{} km / {}",
                            (outcome.distance_m / 1000.0 * 100.0).round() / 100.0,
                            run_page::format_duration(outcome.duration_s)
                        ));
                        log_upload::info(&format!(
                            "跑步提交成功：{} km，用时 {} 秒",
                            (outcome.distance_m / 1000.0).round(),
                            outcome.duration_s
                        ));
                        self.records.stale = true;
                        if self.official.session.is_some() {
                            self.load_records();
                        }
                    }
                    Err(message) => {
                        self.run.step = run_page::RunStep::Preview;
                        self.run.error = Some(message.clone());
                        self.log.push(&format!("失败：{message}"));
                        log_upload::error(&format!("跑步失败：{message}"));
                    }
                }
            }
            Msg::Checkpoints(result) => {
                if let Some(preview) = self.run.preview.as_mut() {
                    preview.checkpoints_loading = false;
                    match result {
                        Ok(points) => {
                            preview.checkpoints = points;
                            preview.checkpoints_error = None;
                        }
                        Err(message) => {
                            preview.checkpoints_error = Some(message);
                        }
                    }
                }
            }
            Msg::Records(result) => {
                self.records.loading = false;
                match result {
                    Ok(rows) => {
                        self.records.error = None;
                        self.records.rows = rows;
                        self.records.stale = false;
                    }
                    Err(message) => {
                        self.records.error = Some(message);
                    }
                }
            }
            Msg::OfficialLogin(result) => {
                self.official.busy = false;
                match result {
                    Ok(session) => {
                        self.official.error = None;
                        self.official.session = Some(session);
                        self.official.open = false;
                        self.toast("官方账号登录成功");
                        if self.run.pending_start {
                            self.run.pending_start = false;
                            self.start_run();
                        }
                        if self.records.stale {
                            self.load_records();
                        }
                    }
                    Err(message) => self.official.error = Some(message),
                }
            }
            Msg::Update(result) => {
                self.update.busy = false;
                self.update_checked = true;
                match result {
                    Ok(Some(release)) => {
                        self.update.release = Some(release);
                        self.update.open = true;
                    }
                    Ok(None) => {
                        self.update.release = None;
                        self.update.open = false;
                    }
                    Err(error) => {
                        self.update.error = Some(error.message);
                    }
                }
            }
            Msg::UpdateProgress(done, total) => {
                self.update.progress = if total == 0 {
                    0.0
                } else {
                    (done as f32 / total as f32).clamp(0.0, 1.0)
                };
            }
            Msg::UpdateDownloaded(result) => {
                self.update.busy = false;
                match result {
                    Ok(path) => {
                        self.update.open = false;
                        crate::platform::install_apk(&path.to_string_lossy());
                    }
                    Err(error) => self.update.error = Some(error.message),
                }
            }
            Msg::LogsUploaded(result) => {
                self.uploading_logs = false;
                if let Err(error) = result {
                    log_upload::warn(&format!("日志上传失败：{error}"));
                }
            }
            Msg::Toast(message) => self.toast(message),
        }
    }

    fn pump_platform(&mut self) {
        #[cfg(target_os = "android")]
        if let Some(reply) = crate::android::take_device_info() {
            match reply.result {
                Ok(Some(info)) => {
                    let identity = crate::api::model::load_identity();
                    match crate::api::model::identity_with_device_info(&identity, &info) {
                        Ok(updated) => {
                            let _ = crate::api::model::save_identity(&updated);
                            self.log.push("已读取本机型号/电量等设备信息");
                        }
                        Err(error) => self.log.push(&format!("设备信息不完整：{error}")),
                    }
                    crate::android::complete_device_info();
                }
                Ok(None) => {}
                Err(message) => {
                    self.log.push(&format!("读取设备信息失败：{message}"));
                    crate::android::complete_device_info();
                }
            }
        }
        let _ = self.device_requested;
    }

    // ── 跑步与记录 ──────────────────────────────────────────

    pub fn load_roads(&mut self) {
        if self.roads.loading {
            return;
        }
        let Some(token) = self.token() else { return };
        self.roads.loading = true;
        self.roads.error = None;
        self.spawn(move |tx| {
            let device_id = crate::soft::session::device_id();
            let result = crate::soft::client::roads(&token, &device_id);
            let _ = tx.send(Msg::Roads(result));
        });
    }

    pub fn load_records(&mut self) {
        let Some(session) = self.official.session.clone() else {
            self.records.error = Some("请先登录官方账号".into());
            return;
        };
        self.records.loading = true;
        self.records.error = None;
        self.spawn(move |tx| {
            let mut client = crate::api::client::ApiClient::new(
                crate::api::model::load_identity(),
                Some(session),
            );
            let result = crate::api::records::fetch_records(&mut client);
            let _ = tx.send(Msg::Records(result));
        });
    }

    /// 拉取当前路网锚点附近的必经点/检查点，标注在预览地图上。
    pub fn load_checkpoints(&mut self) {
        let Some(preview) = self.run.preview.as_mut() else { return };
        if preview.checkpoints_loading {
            return;
        }
        let Some(session) = self.official.session.clone() else {
            preview.checkpoints_error = Some("请先登录官方账号".into());
            self.official.open = true;
            return;
        };
        let anchor = match crate::soft::roads::anchor_bd09(&preview.road) {
            Ok((lat, lon)) => match crate::location::Coordinate::new(lat, lon, 0.0) {
                Ok(c) => c,
                Err(e) => {
                    preview.checkpoints_error = Some(e);
                    return;
                }
            },
            Err(e) => {
                preview.checkpoints_error = Some(e);
                return;
            }
        };
        preview.checkpoints_loading = true;
        preview.checkpoints_error = None;
        self.spawn(move |tx| {
            let mut client = crate::api::client::ApiClient::new(
                crate::api::model::load_identity(),
                Some(session),
            );
            let result = crate::api::points::fetch_points(&mut client, anchor, &mut |_| {})
                .map(|pts| crate::api::points::points_bd(&pts));
            let _ = tx.send(Msg::Checkpoints(result));
        });
    }

    pub fn official_login(&mut self, username: String, password: String) {
        self.official.busy = true;
        self.official.error = None;
        self.spawn(move |tx| {
            let identity = crate::api::model::load_identity();
            let mut client = crate::api::client::ApiClient::new(identity, None);
            let result =
                crate::api::login::login(&mut client, &username, &password, &mut |_| {});
            let _ = tx.send(Msg::OfficialLogin(result));
        });
    }

    /// 正式开跑：校验轮廓 → 上报使用 → 后台执行完整流程。
    pub fn start_run(&mut self) {
        let Some(preview) = self.run.preview.as_ref() else {
            self.run.error = Some("请先选择路网".into());
            return;
        };
        if self.official.session.is_none() {
            self.run.pending_start = true;
            self.official.open = true;
            return;
        }
        let Some(token) = self.token() else { return };
        let mode = self.run.mode;
        // 跑道模式需要可用轮廓；真实路网模式由必经点/检查点驱动，无需轮廓。
        if mode == run_page::RunMode::Track
            && crate::road_editor::validate(&preview.network, &preview.preview.outline).is_err()
        {
            self.run.error = Some("轮廓不可用，请返回调整或恢复默认".into());
            return;
        }
        let road = preview.road.clone();
        let path = preview.path.clone();
        let outline = preview.preview.outline.clone();
        let distance_m = (self.run.distance_km * 1000.0).max(200.0);
        let pace = self.run.pace_sec.clamp(170.0, 520.0);
        let duration_s = ((distance_m / 1000.0) * pace).round().max(60.0) as i64;
        let run_id = uuid::Uuid::new_v4().to_string();
        let version = road.version.clone();
        let device_id = crate::soft::session::device_id();
        let Some(official_session) = self.official.session.clone() else {
            self.run.pending_start = true;
            self.official.open = true;
            return;
        };

        let mode_str = if mode == run_page::RunMode::Road { "road" } else { "track" };
        if let Err(error) = crate::soft::roads::activate(&road, &path, mode_str) {
            self.run.error = Some(error);
            return;
        }
        let mut run_config = crate::api::model::load_config();
        run_config.dist_min = (distance_m / 1000.0) as f32;
        run_config.dist_max = run_config.dist_min + 0.2;
        run_config.pace_min = pace as f32 - 20.0;
        run_config.pace_max = pace as f32 + 20.0;
        let _ = crate::api::model::save_config(&run_config);

        // 需求：本地先记账，上传失败也不丢使用次数。
        crate::soft::roads::queue_use(&road.id, &run_id, &version);

        self.run.running = true;
        self.run.step = run_page::RunStep::Running;
        self.run.progress_text = "正在提交…".into();
        self.run.error = None;
        self.run.result = None;
        self.log.clear();
        self.settings.last_dist_km = self.run.distance_km;
        self.settings.last_pace_sec = pace;
        self.settings.last_road_id = Some(road.id.clone());
        let _ = config::save(&self.settings);
        log_upload::info(&format!(
            "开始跑步：{} · {} km · 配速 {}",
            road.name,
            (distance_m / 1000.0 * 10.0).round() / 10.0,
            run_page::format_pace(pace as f32)
        ));

        let sender = self.tx.clone();
        let ctx = self.ctx.clone();
        let token_clone = token.clone();
        let road_id = road.id.clone();
        let version_clone = version.clone();
        let run_id_clone = run_id.clone();
        std::thread::spawn(move || {
            // 使用统计（失败时保留队列，后台重试）。
            // 若达到每日次数上限，立即终止本次跑步（服务端已限制，本地不再执行）。
            let road_use_result = crate::soft::client::road_use(
                &token_clone,
                &device_id,
                &road_id,
                &run_id_clone,
                &version_clone,
            );
            match &road_use_result {
                Ok((deduped, count)) => {
                    crate::soft::roads::confirm_use(&run_id_clone);
                    let _ = sender.send(Msg::RoadUse(Ok((*deduped, *count))));
                }
                Err(error) => {
                    if error.code == "daily_limit" {
                        let _ = sender.send(Msg::RunDone(Err(error.message.clone())));
                        ctx.request_repaint();
                        return;
                    }
                    let _ = sender.send(Msg::RoadUse(Err(error.clone())));
                }
            }
            ctx.request_repaint();

            let identity = crate::api::model::load_identity();
            let mut client = crate::api::client::ApiClient::new(identity, Some(official_session));
            let params = crate::api::flow::RunParams {
                dist: distance_m,
                dur: duration_s,
                start_ms: chrono::Local::now().timestamp_millis(),
                face_check: 0,
                manual_altitude: None,
                manual_altitude_range: None,
                seed: run_id_clone
                    .bytes()
                    .fold(0u64, |acc, b| acc.wrapping_mul(131).wrapping_add(b as u64)),
                route_mode: if mode == run_page::RunMode::Road {
                    crate::track::generate_road::RouteMode::Road
                } else {
                    crate::track::generate_road::RouteMode::Track
                },
                // 真实路网模式不使用轮廓；跑道模式传入编辑后的轮廓。
                track_outline: if mode == run_page::RunMode::Road {
                    Vec::new()
                } else {
                    outline
                },
                pace_range: Some((pace - 20.0, pace + 20.0)),
                must_limit: None,
            };
            let mut on_log = |line: &str| {
                let _ = sender.send(Msg::RunLog(line.to_string()));
            };
            let result = crate::api::flow::run_full_flow(&mut client, &params, &mut on_log);
            crate::log_upload::flush_local();
            let _ = sender.send(Msg::RunDone(result.map(|_| run_page::RunOutcome {
                distance_m,
                duration_s: duration_s as u64,
            })));
            ctx.request_repaint();
        });
    }

    pub fn renew_prompt(&mut self) {
        self.toast("使用期限已过，请联系管理员续期后点击刷新授权");
    }

    // ── 界面 ────────────────────────────────────────────────

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        egui::TopBottomPanel::top("top_bar")
            .frame(
                egui::Frame::none()
                    .fill(palette.bg)
                    .inner_margin(egui::Margin::symmetric(16.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("RUN").size(22.0).strong().color(palette.accent));
                    ui.label(
                        RichText::new("Miempire's")
                            .size(12.0)
                            .color(palette.text_dim),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let chip = match &self.session {
                            Some(session) if self.can_use() => {
                                format!("{} · {}天", session.display_name(), session.days_left.max(0))
                            }
                            Some(_) => "未授权".to_string(),
                            None => "未登录".to_string(),
                        };
                        let button = egui::Button::new(
                            RichText::new(chip).size(12.5).color(palette.accent),
                        )
                        .fill(palette.accent_soft)
                        .rounding(egui::Rounding::same(12.0));
                        if ui.add(button).clicked() {
                            self.account_open = true;
                        }
                    });
                });
            });
    }

    fn draw_bottom_tabs(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        egui::TopBottomPanel::bottom("bottom_tabs")
            .frame(
                egui::Frame::none()
                    .fill(palette.bg)
                    .inner_margin(egui::Margin::symmetric(16.0, 8.0)),
            )
            .show(ctx, |ui| {
                mobile::tab_bar(ui, &palette, &["跑步", "运动记录"], &mut self.tab);
            });
    }

    fn draw_locked(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(palette.bg).inner_margin(egui::Margin::same(24.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.label(RichText::new("RUN").size(44.0).strong().color(palette.accent));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Miempire's RUN Tool")
                            .size(14.0)
                            .color(palette.text_dim),
                    );
                    ui.add_space(28.0);

                    if self.booting {
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(RichText::new("正在校验授权…").color(palette.text_dim));
                        return;
                    }

                    if let Some(session) = self.session.clone() {
                        egui::Frame::none()
                            .fill(palette.card)
                            .stroke(egui::Stroke::new(1.0, palette.border))
                            .rounding(egui::Rounding::same(18.0))
                            .inner_margin(egui::Margin::same(18.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(session.display_name())
                                        .size(18.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(session.status_text()).color(if session.is_active() {
                                        palette.ok
                                    } else {
                                        palette.err
                                    }),
                                );
                                if let Some(expire) = &session.expires_on {
                                    ui.label(
                                        RichText::new(format!("到期：{}", expire.chars().take(10).collect::<String>()))
                                            .size(12.0)
                                            .color(palette.text_dim),
                                    );
                                }
                                ui.add_space(12.0);
                                let refresh = theme::primary_button(ui, &palette, "刷新授权");
                                if refresh.clicked() {
                                    self.refresh_license();
                                }
                                ui.add_space(6.0);
                                let logout = theme::ghost_button(ui, &palette, "退出登录");
                                if logout.clicked() {
                                    self.logout();
                                }
                            });
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("账号未开通或期限已过，请联系管理员。")
                                .size(12.0)
                                .color(palette.text_dim),
                        );
                    } else {
                        let login = theme::primary_button(ui, &palette, "登录软件账号");
                        if login.clicked() {
                            self.auth.open = true;
                            self.auth.mode = auth::AuthMode::Login;
                            self.auth.error = None;
                        }
                        ui.add_space(6.0);
                        let register = theme::ghost_button(ui, &palette, "注册新账号");
                        if register.clicked() {
                            self.auth.open = true;
                            self.auth.mode = auth::AuthMode::Register;
                            self.auth.error = None;
                        }
                    }

                    ui.add_space(24.0);
                    let address = egui::Button::new(
                        RichText::new(config::base_url())
                            .size(11.0)
                            .color(palette.text_dim),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE);
                    if ui.add(address).clicked() {
                        self.server_input = config::base_url();
                        self.server_open = true;
                    }
                });
            });
    }

    fn draw_main(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(palette.bg).inner_margin(egui::Margin::symmetric(16.0, 8.0)))
            .show(ctx, |ui| {
                match self.tab {
                    0 => run_page::draw_home(self, ui),
                    _ => records_page::draw(self, ui),
                }
            });
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let Some((message, until)) = self.toast.clone() else {
            return;
        };
        if now_secs(ctx) > until {
            self.toast = None;
            return;
        }
        let palette = self.palette;
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -72.0])
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(palette.text.gamma_multiply(0.92))
                    .rounding(egui::Rounding::same(18.0))
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(message).color(Color32::WHITE).size(13.5));
                    });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn draw_server_dialog(&mut self, ctx: &egui::Context) {
        if !self.server_open {
            return;
        }
        let palette = self.palette;
        egui::Window::new("服务器地址")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(300.0);
                ui.label(
                    RichText::new("更换服务器只需修改地址（接口路径随服务器版本更新独立维护）。")
                        .size(12.0)
                        .color(palette.text_dim),
                );
                ui.add_space(6.0);
                mobile::text_edit(ui, "server_input", &mut self.server_input, crate::platform::InputKind::Text, 320.0);
                if let Some(error) = &self.server_error {
                    ui.label(RichText::new(error).color(palette.err).size(12.0));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if theme::primary_button(ui, &palette, "保存").clicked() {
                        match config::server::validate(&self.server_input) {
                            Ok(value) => {
                                self.settings.server_base = value;
                                let _ = config::save(&self.settings);
                                self.server_error = None;
                                self.server_open = false;
                                self.toast("服务器地址已更新");
                            }
                            Err(error) => self.server_error = Some(error),
                        }
                    }
                    if theme::ghost_button(ui, &palette, "取消").clicked() {
                        self.server_open = false;
                    }
                });
                ui.add_space(4.0);
                if theme::ghost_button(ui, &palette, "恢复默认").clicked() {
                    self.server_input = config::server::DEFAULT_BASE.to_string();
                }
            });
    }

    fn draw_about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let palette = self.palette;
        egui::Window::new("关于")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(280.0);
                ui.label(RichText::new("Miempire's RUN Tool").size(18.0).strong().color(palette.accent));
                ui.label(
                    RichText::new(format!("版本 {}", crate::platform::version_name()))
                        .size(12.0)
                        .color(palette.text_dim),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("仅用于个人学习与运动数据记录，请遵守当地法律法规与平台协议。")
                        .size(12.0)
                        .color(palette.text_dim),
                );
                ui.add_space(8.0);
                if theme::primary_button(ui, &palette, "关闭").clicked() {
                    self.about_open = false;
                }
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump();
        self.pump_platform();

        let now = now_secs(ctx);

        // 授权状态每 30 分钟复核一次；启动/操作时已单独刷新。
        if self.can_use() && now - self.last_license_at > 1800.0 {
            self.last_license_at = now;
            if self.session.is_some() {
                self.refresh_license();
            }
        }

        // 日志批量上传（30 秒）。
        if self.can_use() && now - self.last_upload_at > 30.0 {
            self.last_upload_at = now;
            self.upload_logs();
        }

        // 离线使用记录补报（60 秒）。
        if self.can_use() && now - self.last_use_flush_at > 60.0 {
            self.last_use_flush_at = now;
            if crate::soft::roads::has_pending_uses() {
                if let Some(token) = self.token() {
                    self.spawn(move |tx| {
                        let device_id = crate::soft::session::device_id();
                        let result = crate::soft::roads::flush_uses(&token, &device_id);
                        if let Ok(count) = result {
                            if count > 0 {
                                let _ = tx.send(Msg::Toast(format!("已补报 {count} 条路网使用记录")));
                            }
                        }
                    });
                }
            }
        }

        // 自动检查更新（每次启动一次）。
        if self.can_use() && !self.update_checked && now - self.last_update_at > 5.0 {
            self.last_update_at = now;
            self.check_update(false);
        }

        self.pump_platform();

        self.draw_top_bar(ctx);
        if self.can_use() {
            self.draw_bottom_tabs(ctx);
            self.draw_main(ctx);
        } else {
            self.draw_locked(ctx);
        }
        self.draw_toast(ctx);

        auth::draw_soft_auth(self, ctx);
        auth::draw_account_sheet(self, ctx);
        run_page::draw_dialog(self, ctx);
        update_dialog::draw(self, ctx);
        self.draw_server_dialog(ctx);
        self.draw_about(ctx);

        // 主题切换后即时生效。
        if self.theme != ThemeKind::parse(&self.settings.theme) {
            self.settings.theme = self.theme.as_str().to_string();
            let _ = config::save(&self.settings);
            self.palette = self.theme.palette();
            self.palette.apply(ctx);
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        crate::platform::apply_safe_area(ctx, raw_input);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        log_upload::flush_local();
    }
}
