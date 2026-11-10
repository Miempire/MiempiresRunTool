//! 运动记录页：官方账号登录后展示跑步记录。

use chrono::TimeZone;
use eframe::egui;
use egui::{Align, Color32, Layout, RichText};

use super::run_page::{format_duration, format_pace};
use super::{theme, App};
use crate::api::records::RecordRow;

pub struct RecordsUi {
    pub rows: Vec<RecordRow>,
    pub loading: bool,
    pub error: Option<String>,
    /// true 表示需要重新拉取（首次进入或刚完成一次跑步）。
    pub stale: bool,
}

impl Default for RecordsUi {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            loading: false,
            error: None,
            stale: true,
        }
    }
}

fn dim(text: impl Into<String>, palette: &theme::Palette) -> RichText {
    RichText::new(text.into())
        .size(11.5)
        .color(palette.text_dim)
}

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;

    if app.official.session.is_none() {
        ui.add_space(8.0);
        theme::card_frame(&palette).show(ui, |ui| {
            ui.label(
                RichText::new("运动记录")
                    .size(18.0)
                    .strong()
                    .color(palette.text),
            );
            ui.add_space(4.0);
            ui.label(dim(
                "绑定官方运动账号后，这里会显示每次跑步的成绩。",
                &palette,
            ));
            ui.add_space(10.0);
            let width = ui.available_width();
            let button = egui::Button::new(
                RichText::new("登录官方账号").color(Color32::WHITE).size(16.0),
            )
            .fill(palette.accent)
            .rounding(egui::Rounding::same(14.0));
            if ui.add_sized([width, 44.0], button).clicked() {
                app.official.open = true;
                app.official.error = None;
            }
        });
        return;
    }

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("运动记录")
                .size(18.0)
                .strong()
                .color(palette.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::ghost_button(ui, &palette, "刷新").clicked() {
                app.records.error = None;
                app.load_records();
            }
        });
    });

    if app.records.stale && !app.records.loading && app.records.error.is_none() {
        app.load_records();
    }

    if app.records.loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("正在获取记录…").color(palette.text_dim));
        });
    }
    if let Some(error) = app.records.error.clone() {
        ui.label(RichText::new(error).size(12.0).color(palette.err));
    }

    if !app.records.loading && app.records.error.is_none() && app.records.rows.is_empty() {
        ui.add_space(8.0);
        ui.label(dim("还没有跑步记录，先去跑一场吧。", &palette));
    }

    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for row in &app.records.rows {
                let when = chrono::Local
                    .timestamp_millis_opt(row.start_time)
                    .single()
                    .map(|dt| dt.format("%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "--".into());
                let pace = if row.total_dis > 0.0 {
                    row.total_time as f64 / (row.total_dis / 1000.0)
                } else {
                    0.0
                };
                theme::card_frame(&palette).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("{:.2} km", row.total_dis / 1000.0))
                                    .size(18.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.label(dim(
                                format!(
                                    "{} · 用时 {}",
                                    when,
                                    format_duration(row.total_time.max(0) as u64)
                                ),
                                &palette,
                            ));
                            ui.label(dim(
                                format!(
                                    "配速 {} · {} 千卡 · {} 步",
                                    format_pace(pace as f32),
                                    row.calorie.max(0),
                                    row.total_steps.max(0)
                                ),
                                &palette,
                            ));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (text, color) = if row.complete {
                                ("已完成", palette.ok)
                            } else {
                                ("未完成", palette.err)
                            };
                            ui.label(RichText::new(text).size(12.0).color(color));
                        });
                    });
                });
                ui.add_space(2.0);
            }
        });
}
