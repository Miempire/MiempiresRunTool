//! 跑步主页与跑步弹窗。
//!
//! 主界面只保留一个入口；「选路网 → 预览 → 编辑轮廓 → 参数 → 运行 → 完成」
//! 全部收在弹窗里，避免把繁杂配置暴露给新用户。

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use egui::{Align, Color32, Layout, RichText};

use super::map::{self, MapState};
use super::{theme, App, Msg};
use crate::road_editor;
use crate::soft::model::RoadInfo;
use crate::track::geom::{MET_PER_DEG_LAT, MET_PER_DEG_LNG};

#[derive(Default)]
pub struct RoadsUi {
    pub list: Vec<RoadInfo>,
    pub loading: bool,
    pub error: Option<String>,
}

/// 跑步模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// 真实道路路由：沿道路依次串联必经点/检查点。
    Road,
    /// 跑道绕圈：在选定路网内绕封闭轮廓。
    Track,
}

impl RunMode {
    pub fn label(self) -> &'static str {
        match self {
            RunMode::Road => "真实路网",
            RunMode::Track => "跑道绕圈",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStep {
    PickRoad,
    Preview,
    Edit,
    Running,
    Done,
}

pub struct PreviewState {
    pub road: RoadInfo,
    pub network: Arc<route_planner::RoadGraph>,
    pub path: PathBuf,
    pub preview: road_editor::Preview,
    pub selected_vertex: Option<usize>,
    pub validation: Result<(), String>,
    pub map: MapState,
    pub last_map_key: String,
    /// 候选跑道当前页（每页 6 条）。
    pub candidate_page: usize,
    /// 必经点/检查点（BD09），加载成功后标注在地图上。
    pub checkpoints: Vec<(f64, f64)>,
    pub checkpoints_loading: bool,
    pub checkpoints_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub distance_m: f64,
    pub duration_s: u64,
}

pub struct RunUi {
    pub open: bool,
    pub step: RunStep,
    pub preparing: bool,
    pub download: f32,
    pub selected: Option<RoadInfo>,
    pub preview: Option<PreviewState>,
    pub error: Option<String>,
    pub running: bool,
    pub progress_text: String,
    pub result: Option<RunOutcome>,
    pub pending_start: bool,
    pub distance_km: f64,
    pub pace_sec: f64,
    /// 跑步模式（真实路网 / 跑道绕圈）。
    pub mode: RunMode,
    /// 正在拖动的轮廓顶点（仅在编辑页使用）。
    pub drag: Option<usize>,
}

impl RunUi {
    pub fn new(settings: &crate::config::Settings) -> Self {
        Self {
            open: false,
            step: RunStep::PickRoad,
            preparing: false,
            download: 0.0,
            selected: None,
            preview: None,
            error: None,
            running: false,
            progress_text: String::new(),
            result: None,
            pending_start: false,
            distance_km: settings.last_dist_km.clamp(0.5, 21.0),
            pace_sec: settings.last_pace_sec.clamp(170.0, 520.0),
            mode: RunMode::Track,
            drag: None,
        }
    }

    pub fn reset(&mut self) {
        self.open = false;
        self.step = RunStep::PickRoad;
        self.preparing = false;
        self.download = 0.0;
        self.selected = None;
        self.preview = None;
        self.error = None;
        self.running = false;
        self.progress_text.clear();
        self.result = None;
        self.pending_start = false;
        self.drag = None;
    }
}

pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

pub fn format_pace(sec_per_km: f32) -> String {
    if !sec_per_km.is_finite() || sec_per_km <= 0.0 {
        return "--'--\"".into();
    }
    let total = sec_per_km.round() as u64;
    format!("{}'{:02}\"", total / 60, total % 60)
}

fn dim(text: impl Into<String>, palette: &theme::Palette) -> RichText {
    RichText::new(text.into())
        .size(11.5)
        .color(palette.text_dim)
}

fn open_dialog(app: &mut App) {
    app.run.open = true;
    app.run.error = None;
    app.run.result = None;
    if app.run.preview.is_some() {
        app.run.step = RunStep::Preview;
    } else {
        app.run.step = RunStep::PickRoad;
        app.load_roads();
    }
}

// ── 主页 ────────────────────────────────────────────────────

pub fn draw_home(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    ui.add_space(4.0);

    theme::card_frame(&palette).show(ui, |ui| {
        ui.label(
            RichText::new("今天，跑一场")
                .size(21.0)
                .strong()
                .color(palette.text),
        );
        ui.label(dim("选好路网就能开始，其余交给软件。", &palette));
        ui.add_space(10.0);

        match app.run.selected.as_ref() {
            Some(road) => {
                let city = if road.city.trim().is_empty() {
                    "未标注城市".to_string()
                } else {
                    road.city.clone()
                };
                ui.label(
                    RichText::new(format!("{} · {}", road.name, city))
                        .size(13.5)
                        .color(palette.text),
                );
                ui.label(
                    RichText::new(format!("{} · 已使用 {} 次", road.author(), road.usage_count))
                        .size(11.5)
                        .color(palette.accent),
                );
            }
            None => {
                ui.label(dim("还没有选择路网", &palette));
            }
        }

        ui.add_space(12.0);
        let width = ui.available_width();
        let button = egui::Button::new(
            RichText::new("开始跑步").color(Color32::WHITE).size(17.0),
        )
        .fill(palette.accent)
        .rounding(egui::Rounding::same(16.0));
        if ui.add_sized([width, 52.0], button).clicked() {
            open_dialog(app);
        }

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(dim(
                format!("{:.1} km", app.run.distance_km),
                &palette,
            ));
            ui.label(dim("·", &palette));
            ui.label(dim(
                format!("配速 {}", format_pace(app.run.pace_sec as f32)),
                &palette,
            ));
            ui.label(dim("·", &palette));
            ui.label(dim(
                format!(
                    "预计 {}",
                    format_duration(
                        (app.run.distance_km * app.run.pace_sec).round().max(1.0) as u64
                    )
                ),
                &palette,
            ));
        });
    });

    ui.add_space(10.0);
    theme::card_frame(&palette).show(ui, |ui| {
        ui.label(
            RichText::new("三步开跑")
                .size(13.5)
                .strong()
                .color(palette.text),
        );
        ui.add_space(2.0);
        for (index, text) in [
            "选择服务器上的路网",
            "预览并微调轮廓",
            "确认距离与配速，开始",
        ]
        .iter()
        .enumerate()
        {
            ui.label(dim(format!("{}. {}", index + 1, text), &palette));
        }
    });

    if app.official.session.is_none() {
        ui.add_space(10.0);
        theme::card_frame(&palette).show(ui, |ui| {
            ui.label(
                RichText::new("首次使用需要绑定官方账号")
                    .size(12.5)
                    .color(palette.warn),
            );
            ui.label(dim(
                "点击开始后按提示登录，登录状态会加密保存在本机。",
                &palette,
            ));
        });
    }
}

// ── 弹窗 ────────────────────────────────────────────────────

pub fn draw_dialog(app: &mut App, ctx: &egui::Context) {
    if !app.run.open {
        return;
    }
    let palette = app.palette;
    let title = match app.run.step {
        RunStep::PickRoad => "跑步 · 选择路网",
        RunStep::Preview => "跑步 · 预览",
        RunStep::Edit => "跑步 · 编辑轮廓",
        RunStep::Running => "跑步中",
        RunStep::Done => "跑步完成",
    };
    let max_height = (ctx.screen_rect().height() * 0.74).max(300.0);
    let mut close = false;

    egui::Window::new(title)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.horizontal(|ui| {
                ui.label(dim(step_text(app.run.step), &palette));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !app.run.running {
                        let button = egui::Button::new(
                            RichText::new("关闭").size(11.5).color(palette.text_dim),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE);
                        if ui.add(button).clicked() {
                            close = true;
                        }
                    }
                });
            });
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    match app.run.step {
                        RunStep::PickRoad => draw_pick_road(app, ui),
                        RunStep::Preview => draw_preview(app, ui),
                        RunStep::Edit => draw_edit(app, ui),
                        RunStep::Running => draw_running(app, ui),
                        RunStep::Done => draw_done(app, ui),
                    }

                    if let Some(error) = app.run.error.clone() {
                        ui.add_space(6.0);
                        ui.label(RichText::new(error).size(12.0).color(palette.err));
                    }
                });
        });

    if close {
        app.run.open = false;
    }
}

fn step_text(step: RunStep) -> &'static str {
    match step {
        RunStep::PickRoad => "1 / 3 · 选择服务器路网",
        RunStep::Preview => "2 / 3 · 预览与参数",
        RunStep::Edit => "轮廓编辑",
        RunStep::Running => "正在提交，请保持应用在前台",
        RunStep::Done => "本次跑步已提交",
    }
}

// ── 步骤 1：选择路网 ────────────────────────────────────────

fn draw_pick_road(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;

    if app.roads.loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("正在获取路网…").color(palette.text_dim));
        });
    }
    if let Some(error) = app.roads.error.clone() {
        ui.label(RichText::new(error).size(12.0).color(palette.err));
    }

    ui.horizontal(|ui| {
        if theme::ghost_button(ui, &palette, "刷新列表").clicked() {
            app.load_roads();
        }
        if !app.roads.list.is_empty() {
            ui.label(dim(format!("共 {} 个", app.roads.list.len()), &palette));
        }
    });

    if app.run.preparing {
        ui.add_space(4.0);
        ui.label(dim("正在下载并解析路网…", &palette));
        ui.add(egui::ProgressBar::new(app.run.download).show_percentage());
    }

    if app.roads.list.is_empty() {
        if !app.roads.loading && app.roads.error.is_none() {
            ui.add_space(8.0);
            ui.label(dim(
                "服务器还没有发布路网，请联系管理员添加。",
                &palette,
            ));
        }
        return;
    }

    ui.add_space(4.0);
    let mut pick: Option<RoadInfo> = None;
    egui::ScrollArea::vertical()
        .max_height(330.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for road in &app.roads.list {
                let selected = app
                    .run
                    .selected
                    .as_ref()
                    .map(|item| item.id == road.id)
                    .unwrap_or(false);
                theme::card_frame(&palette).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&road.name)
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            let city = if road.city.trim().is_empty() {
                                "未标注城市".to_string()
                            } else {
                                road.city.clone()
                            };
                            ui.label(dim(
                                format!(
                                    "{} · {} · {}",
                                    city,
                                    road.author(),
                                    road.size_text()
                                ),
                                &palette,
                            ));
                            ui.label(
                                RichText::new(format!("已使用 {} 次", road.usage_count))
                                    .size(11.5)
                                    .color(palette.accent),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let label = if selected { "已选择" } else { "选择" };
                            let button = egui::Button::new(
                                RichText::new(label)
                                    .size(13.0)
                                    .color(Color32::WHITE),
                            )
                            .fill(palette.accent)
                            .rounding(egui::Rounding::same(12.0))
                            .min_size(egui::vec2(76.0, 36.0));
                            if ui.add_enabled(!app.run.preparing, button).clicked() {
                                pick = Some(road.clone());
                            }
                        });
                    });
                });
                ui.add_space(2.0);
            }
        });

    if let Some(road) = pick {
        prepare_road(app, road);
    }
}

fn prepare_road(app: &mut App, road: RoadInfo) {
    if app.run.preparing {
        return;
    }
    let Some(token) = app.token() else {
        app.run.error = Some("请先登录软件账号".into());
        return;
    };
    app.run.preparing = true;
    app.run.download = 0.0;
    app.run.error = None;

    app.spawn(move |tx| {
        let device_id = crate::soft::session::device_id();
        let progress_tx = tx.clone();
        let fetched = crate::soft::roads::ensure(&road, &token, &device_id, move |done, total| {
            let _ = progress_tx.send(Msg::RoadProgress(done, total));
        });
        let (network, preview, path) = match fetched {
            Ok(path) => match crate::soft::roads::load_network(&path) {
                Ok(network) => {
                    let preview = road_editor::build_preview(&network, 0).map(Box::new);
                    (Some(Arc::new(network)), preview, Some(path))
                }
                Err(message) => (None, Err(message), Some(path)),
            },
            Err(error) => (None, Err(error.message), None),
        };
        let _ = tx.send(Msg::RoadPrepared {
            road,
            preview,
            network,
            path,
        });
    });
}

// ── 步骤 2：预览 ────────────────────────────────────────────

fn draw_preview(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;

    let summary = app.run.preview.as_ref().map(|state| {
        (
            state.road.clone(),
            state.preview.candidates.clone(),
            state.preview.selected,
            road_editor::outline_length_m(&state.preview.outline),
            state.preview.outline.len(),
            state.validation.clone(),
        )
    });
    let Some((road, candidates, selected, length, vertices, validation)) = summary else {
        ui.label(dim("请先选择路网", &palette));
        return;
    };
    let page = app
        .run
        .preview
        .as_ref()
        .map(|state| state.candidate_page)
        .unwrap_or(0);
    let (cp_count, cp_loading, cp_error) = app
        .run
        .preview
        .as_ref()
        .map(|state| {
            (
                state.checkpoints.len(),
                state.checkpoints_loading,
                state.checkpoints_error.clone(),
            )
        })
        .unwrap_or((0, false, None));

    let city = if road.city.trim().is_empty() {
        "未标注城市".to_string()
    } else {
        road.city.clone()
    };
    ui.label(
        RichText::new(format!("{} · {}", road.name, city))
            .size(15.0)
            .strong()
            .color(palette.text),
    );
    let subtitle = match app.run.mode {
        RunMode::Road => format!("{} · 已使用 {} 次", road.author(), road.usage_count),
        RunMode::Track => format!(
            "{} · 已使用 {} 次 · {} 个顶点 · {:.0} m",
            road.author(),
            road.usage_count,
            vertices,
            length
        ),
    };
    ui.label(dim(subtitle, &palette));

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for mode in [RunMode::Road, RunMode::Track] {
            let active = app.run.mode == mode;
            let button = egui::Button::new(
                RichText::new(mode.label()).size(12.5).color(if active {
                    Color32::WHITE
                } else {
                    palette.accent
                }),
            )
            .fill(if active {
                palette.accent
            } else {
                palette.accent_soft
            })
            .rounding(egui::Rounding::same(10.0));
            if ui.add(button).clicked() {
                app.run.mode = mode;
            }
        }
    });
    ui.label(dim(
        match app.run.mode {
            RunMode::Road => "沿真实道路依次串联必经点与检查点。",
            RunMode::Track => "在路网内绕封闭跑道，可挑选候选或手动编辑轮廓。",
        },
        &palette,
    ));

    if app.run.mode == RunMode::Track {
        if candidates.is_empty() {
            ui.add_space(2.0);
            ui.label(
                RichText::new("未识别到封闭跑道，请手动编辑轮廓")
                    .size(12.0)
                    .color(palette.warn),
            );
        } else {
            let total_pages = ((candidates.len() + 5) / 6).max(1);
            let page = page.min(total_pages - 1);
            let start_index = page * 6;
            ui.add_space(2.0);
            ui.label(dim(
                format!(
                    "自动识别到 {} 条封闭跑道 · 第 {}/{} 页",
                    candidates.len(),
                    page + 1,
                    total_pages
                ),
                &palette,
            ));
            let mut choose: Option<usize> = None;
            let mut next_page = false;
            ui.horizontal_wrapped(|ui| {
                for (offset, candidate) in
                    candidates.iter().skip(start_index).take(6).enumerate()
                {
                    let index = start_index + offset;
                    let active = selected == Some(index);
                    let button = egui::Button::new(
                        RichText::new(format!("{} · {:.0}m", index + 1, candidate.length_m))
                            .size(11.5)
                            .color(if active {
                                Color32::WHITE
                            } else {
                                palette.accent
                            }),
                    )
                    .fill(if active {
                        palette.accent
                    } else {
                        palette.accent_soft
                    })
                    .rounding(egui::Rounding::same(10.0));
                    if ui.add(button).clicked() {
                        choose = Some(index);
                    }
                }
                if total_pages > 1 && theme::ghost_button(ui, &palette, "换一批").clicked() {
                    next_page = true;
                }
            });
            if let Some(state) = app.run.preview.as_mut() {
                state.candidate_page = if next_page {
                    (page + 1) % total_pages
                } else {
                    page
                };
                if let Some(index) = choose {
                    road_editor::choose(&mut state.preview, index);
                    state.selected_vertex = None;
                    state.validation =
                        road_editor::validate(&state.network, &state.preview.outline);
                }
            }
        }
    }

    ui.add_space(4.0);
    draw_map(app, ui, false);
    ui.add_space(4.0);

    let mut load_points = false;
    ui.horizontal(|ui| {
        if theme::ghost_button(ui, &palette, "加载必经点/检查点").clicked() {
            load_points = true;
        }
        if cp_loading {
            ui.spinner();
        } else if let Some(error) = &cp_error {
            ui.label(RichText::new(error).size(11.5).color(palette.err));
        } else if cp_count > 0 {
            ui.label(dim(format!("已在地图上标注 {} 个检查点", cp_count), &palette));
        }
    });
    if load_points {
        app.load_checkpoints();
    }

    if app.run.mode == RunMode::Track {
        match &validation {
            Ok(()) if vertices >= 3 => {
                ui.label(
                    RichText::new("轮廓校验通过")
                        .size(12.0)
                        .color(palette.ok),
                );
            }
            Ok(()) => {
                ui.label(
                    RichText::new("轮廓顶点不足，请编辑后再试")
                        .size(12.0)
                        .color(palette.warn),
                );
            }
            Err(message) => {
                ui.label(RichText::new(message).size(12.0).color(palette.err));
            }
        }
    }

    ui.add_space(2.0);
    ui.add(
        egui::Slider::new(&mut app.run.distance_km, 0.5..=21.0)
            .step_by(0.5)
            .text("距离 (km)"),
    );
    ui.add(
        egui::Slider::new(&mut app.run.pace_sec, 170.0..=520.0)
            .step_by(5.0)
            .text("配速")
            .custom_formatter(|value, _| format_pace(value as f32)),
    );
    ui.label(dim(
        format!(
            "预计用时 {}",
            format_duration(
                (app.run.distance_km * app.run.pace_sec).round().max(1.0) as u64
            )
        ),
        &palette,
    ));

    ui.add_space(6.0);
    let mut start = false;
    ui.horizontal(|ui| {
        if theme::ghost_button(ui, &palette, "重选路网").clicked() {
            app.run.step = RunStep::PickRoad;
        }
        if app.run.mode == RunMode::Track
            && theme::ghost_button(ui, &palette, "编辑轮廓").clicked()
        {
            app.run.step = RunStep::Edit;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::primary_button(ui, &palette, "开始跑步").clicked() {
                start = true;
            }
        });
    });
    if start {
        app.start_run();
    }
}

// ── 步骤 3：编辑轮廓 ────────────────────────────────────────

fn draw_edit(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    ui.label(dim(
        "拖动顶点调整；点击线段插入新点；选中顶点后可删除。",
        &palette,
    ));
    ui.add_space(4.0);
    draw_map(app, ui, true);
    ui.add_space(6.0);

    let selected = app.run.preview.as_ref().and_then(|state| state.selected_vertex);
    let (mut snap, mut smooth, mut reverse, mut remove, mut restore, mut save, mut back) = (
        false, false, false, false, false, false, false,
    );

    ui.horizontal_wrapped(|ui| {
        if theme::ghost_button(ui, &palette, "吸附到道路").clicked() {
            snap = true;
        }
        if theme::ghost_button(ui, &palette, "平滑").clicked() {
            smooth = true;
        }
        if theme::ghost_button(ui, &palette, "反向").clicked() {
            reverse = true;
        }
        let delete =
            egui::Button::new(RichText::new("删除顶点").size(15.0).color(palette.accent))
                .fill(palette.card)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .rounding(egui::Rounding::same(14.0))
                .min_size(egui::vec2(0.0, 40.0));
        if ui.add_enabled(selected.is_some(), delete).clicked() {
            remove = true;
        }
    });

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        if theme::ghost_button(ui, &palette, "恢复默认").clicked() {
            restore = true;
        }
        if theme::ghost_button(ui, &palette, "返回预览").clicked() {
            back = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::primary_button(ui, &palette, "保存轮廓").clicked() {
                save = true;
            }
        });
    });

    if let Some(state) = app.run.preview.as_mut() {
        if snap {
            road_editor::snap_all(&state.network, &mut state.preview.outline, 25.0);
            state.preview.selected = None;
        }
        if smooth {
            road_editor::smooth(&mut state.preview.outline);
            state.preview.selected = None;
        }
        if reverse {
            road_editor::reverse(&mut state.preview.outline);
            state.preview.selected = None;
        }
        if remove {
            if let Some(index) = state.selected_vertex {
                if road_editor::remove_vertex(&mut state.preview.outline, index) {
                    state.selected_vertex = None;
                }
            }
        }
        if restore && !state.preview.candidates.is_empty() {
            road_editor::choose(&mut state.preview, 0);
            state.selected_vertex = None;
        }
        if snap || smooth || reverse || remove || restore || save {
            state.validation = road_editor::validate(&state.network, &state.preview.outline);
        }
        if save && state.validation.is_ok() {
            app.run.step = RunStep::Preview;
        }
    }
    if back {
        app.run.step = RunStep::Preview;
    }

    let validation = app
        .run
        .preview
        .as_ref()
        .map(|state| state.validation.clone())
        .unwrap_or(Ok(()));
    match &validation {
        Ok(()) => {
            let vertices = app
                .run
                .preview
                .as_ref()
                .map(|state| state.preview.outline.len())
                .unwrap_or(0);
            if vertices >= 3 {
                ui.label(RichText::new("轮廓可用").size(12.0).color(palette.ok));
            } else {
                ui.label(
                    RichText::new("至少需要 3 个顶点")
                        .size(12.0)
                        .color(palette.warn),
                );
            }
        }
        Err(message) => {
            ui.label(RichText::new(message).size(12.0).color(palette.err));
        }
    }
}

// ── 步骤 4：运行 / 完成 ─────────────────────────────────────

fn draw_running(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(
            RichText::new(if app.run.progress_text.is_empty() {
                "正在提交…"
            } else {
                app.run.progress_text.as_str()
            })
            .color(palette.text_dim),
        );
    });
    ui.add_space(6.0);
    ui.label(dim(
        "请保持应用在前台，完成后会自动刷新运动记录。",
        &palette,
    ));
    ui.add_space(6.0);
    if let Some(last) = app.log.last().map(|line| line.to_string()) {
        ui.label(dim(last, &palette));
    }
}

fn draw_done(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let Some(result) = app.run.result.clone() else {
        return;
    };
    let pace = result.duration_s as f64 / (result.distance_m / 1000.0).max(0.001);

    theme::card_frame(&palette).show(ui, |ui| {
        ui.label(
            RichText::new("跑步完成")
                .size(18.0)
                .strong()
                .color(palette.ok),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("{:.2} km", result.distance_m / 1000.0))
                .size(26.0)
                .strong()
                .color(palette.text),
        );
        ui.label(dim(
            format!(
                "用时 {} · 平均配速 {}",
                format_duration(result.duration_s),
                format_pace(pace as f32)
            ),
            &palette,
        ));
    });

    ui.add_space(8.0);
    let width = ui.available_width();
    let button =
        egui::Button::new(RichText::new("查看运动记录").color(Color32::WHITE).size(16.0))
            .fill(palette.accent)
            .rounding(egui::Rounding::same(14.0));
    if ui.add_sized([width, 44.0], button).clicked() {
        app.run.open = false;
        app.run.step = RunStep::PickRoad;
        app.run.result = None;
        app.tab = 1;
        app.load_records();
    }
    ui.add_space(4.0);
    let again = egui::Button::new(
        RichText::new("再跑一次").size(15.0).color(palette.accent),
    )
    .fill(palette.card)
    .stroke(egui::Stroke::new(1.0, palette.border))
    .rounding(egui::Rounding::same(14.0));
    if ui.add_sized([width, 42.0], again).clicked() {
        app.run.result = None;
        app.run.error = None;
        app.run.step = if app.run.preview.is_some() {
            RunStep::Preview
        } else {
            RunStep::PickRoad
        };
    }
}

// ── 地图与编辑交互 ──────────────────────────────────────────

fn screen_to_deg(map: &MapState, rect: egui::Rect, pos: egui::Pos2) -> (f64, f64) {
    let center = rect.center();
    let scale = map.scale.max(0.0001) as f64;
    let x = (pos.x - center.x - map.offset.x) as f64 / scale;
    let y = (pos.y - center.y - map.offset.y) as f64 / scale;
    (
        map.center.0 - y / MET_PER_DEG_LAT,
        map.center.1 + x / MET_PER_DEG_LNG,
    )
}

fn draw_map(app: &mut App, ui: &mut egui::Ui, editable: bool) {
    let palette = app.palette;
    let width = ui.available_width().clamp(240.0, 420.0);
    let height = (width * 0.68).clamp(220.0, 340.0);
    let (response, painter) = ui.allocate_painter(
        egui::vec2(width, height),
        egui::Sense::click_and_drag(),
    );
    let rect = response.rect;
    painter.rect_filled(rect, egui::Rounding::same(14.0), palette.card);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(14.0),
        egui::Stroke::new(1.0, palette.border),
    );

    let mode = app.run.mode;
    let Some(state) = app.run.preview.as_mut() else {
        return;
    };

    let key = format!(
        "{}:{}",
        if editable { "edit" } else { "preview" },
        state.preview.candidates.len()
    );
    if state.last_map_key != key {
        let mut points = map::collect_bounds(&state.preview.edges);
        if points.is_empty() {
            points = state.preview.outline.clone();
        }
        if points.is_empty() {
            points.push((state.road.center_lat, state.road.center_lon));
        }
        state.map.fit(rect, &points);
        if points.len() == 1 {
            state.map.center = points[0];
        }
        state.last_map_key = key;
    }

    if editable {
        let pointer = ui.input(|input| input.pointer.interact_pos());
        let down = ui.input(|input| input.pointer.primary_down());

        if response.drag_started() {
            if let Some(pos) = pointer {
                let tol = (18.0 / state.map.scale.max(0.0001) as f64).clamp(5.0, 60.0);
                let world = screen_to_deg(&state.map, rect, pos);
                if let Some(index) =
                    road_editor::hit_vertex(&state.preview.outline, world, tol)
                {
                    state.selected_vertex = Some(index);
                    app.run.drag = Some(index);
                } else if let Some(segment) =
                    road_editor::hit_segment(&state.preview.outline, world, tol)
                {
                    let point =
                        road_editor::snap(&state.network, world, 30.0).unwrap_or(world);
                    road_editor::insert_vertex(&mut state.preview.outline, segment, point);
                    state.selected_vertex = Some(segment + 1);
                    state.preview.selected = None;
                    app.run.drag = Some(segment + 1);
                } else {
                    state.selected_vertex = None;
                }
            }
        }

        if let Some(index) = app.run.drag {
            if down {
                if let Some(pos) = pointer {
                    let world = screen_to_deg(&state.map, rect, pos);
                    let point =
                        road_editor::snap(&state.network, world, 30.0).unwrap_or(world);
                    if let Some(vertex) = state.preview.outline.get_mut(index) {
                        *vertex = point;
                        state.preview.selected = None;
                    }
                }
            } else {
                app.run.drag = None;
                state.validation =
                    road_editor::validate(&state.network, &state.preview.outline);
            }
        } else if response.clicked() {
            if let Some(pos) = pointer {
                let tol = (16.0 / state.map.scale.max(0.0001) as f64).clamp(4.0, 45.0);
                let world = screen_to_deg(&state.map, rect, pos);
                if let Some(index) =
                    road_editor::hit_vertex(&state.preview.outline, world, tol)
                {
                    state.selected_vertex = Some(index);
                } else if let Some(segment) =
                    road_editor::hit_segment(&state.preview.outline, world, tol)
                {
                    let point =
                        road_editor::snap(&state.network, world, 30.0).unwrap_or(world);
                    road_editor::insert_vertex(&mut state.preview.outline, segment, point);
                    state.selected_vertex = Some(segment + 1);
                    state.preview.selected = None;
                    state.validation =
                        road_editor::validate(&state.network, &state.preview.outline);
                } else {
                    state.selected_vertex = None;
                }
            }
        } else {
            state.map.interact(ui, rect);
        }
    } else {
        state.map.interact(ui, rect);
    }

    for edge in &state.preview.edges {
        state
            .map
            .draw_polyline(&painter, rect, edge, palette.map_road, 1.5);
    }
    for building in &state.preview.buildings {
        state
            .map
            .draw_polygon(&painter, rect, building, palette.map_building, 1.0);
    }

    if mode == RunMode::Track {
        let outline = &state.preview.outline;
        if outline.len() >= 3 {
            state
                .map
                .draw_polygon(&painter, rect, outline, palette.map_outline, 3.0);
        } else if outline.len() == 2 {
            state
                .map
                .draw_polyline(&painter, rect, outline, palette.map_outline, 3.0);
        }
        for (index, (lat, lon)) in outline.iter().enumerate() {
            let selected = state.selected_vertex == Some(index);
            let radius = if selected { 7.0 } else { 5.0 };
            let fill = if selected {
                palette.accent
            } else {
                palette.map_vertex
            };
            state.map.draw_point(&painter, rect, *lat, *lon, fill, radius);
            let screen = state.map.to_screen(rect, *lat, *lon);
            painter.circle_stroke(
                screen,
                radius,
                egui::Stroke::new(1.5, palette.map_outline),
            );
        }
    }

    // 必经点/检查点标注（BD09）：橙色圆点 + 序号。
    let checkpoint = Color32::from_rgb(255, 122, 26);
    for (index, (lat, lon)) in state.checkpoints.iter().enumerate() {
        let screen = state.map.to_screen(rect, *lat, *lon);
        painter.circle_filled(screen, 7.5, checkpoint);
        painter.circle_stroke(screen, 7.5, egui::Stroke::new(1.5, Color32::WHITE));
        painter.text(
            screen,
            egui::Align2::CENTER_CENTER,
            format!("{}", index + 1),
            egui::FontId::proportional(9.0),
            Color32::WHITE,
        );
    }
}
