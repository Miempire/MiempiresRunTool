//! 客户端更新弹窗：展示版本说明、下载进度并交给系统安装器。

use eframe::egui;
use egui::RichText;

use super::{theme, App, Msg};

#[derive(Default)]
pub struct UpdateUi {
    pub release: Option<crate::soft::model::ReleaseInfo>,
    pub open: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub progress: f32,
}

fn dim(text: impl Into<String>, palette: &theme::Palette) -> RichText {
    RichText::new(text.into())
        .size(11.5)
        .color(palette.text_dim)
}

pub fn draw(app: &mut App, ctx: &egui::Context) {
    if !app.update.open {
        return;
    }
    let Some(release) = app.update.release.clone() else {
        return;
    };
    let palette = app.palette;
    let max_height = (ctx.screen_rect().height() * 0.5).max(180.0);
    let mut download = false;
    let mut later = false;

    egui::Window::new("发现新版本")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(330.0)
        .show(ctx, |ui| {
            ui.set_max_width(330.0);
            ui.label(
                RichText::new(format!(
                    "{} → {}",
                    crate::platform::version_name(),
                    release.version
                ))
                .size(13.0)
                .color(palette.text_dim),
            );
            if release.size > 0 {
                ui.label(dim(
                    format!("安装包 {:.1} MB", release.size as f64 / 1_048_576.0),
                    &palette,
                ));
            }
            if !release.notes.trim().is_empty() {
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(max_height.min(160.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&release.notes)
                                .size(12.0)
                                .color(palette.text),
                        );
                    });
            }
            if release.force {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("此版本为强制更新，完成后才能继续使用。")
                        .size(12.0)
                        .color(palette.warn),
                );
            }

            if app.update.busy {
                ui.add_space(6.0);
                ui.add(egui::ProgressBar::new(app.update.progress).show_percentage());
            }
            if let Some(error) = app.update.error.clone() {
                ui.add_space(4.0);
                ui.label(RichText::new(error).size(12.0).color(palette.err));
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = if app.update.busy {
                    "下载中…"
                } else {
                    "立即更新"
                };
                if theme::primary_button(ui, &palette, label).clicked() && !app.update.busy {
                    download = true;
                }
                if !release.force && theme::ghost_button(ui, &palette, "稍后").clicked() {
                    later = true;
                }
            });
        });

    if later {
        app.update.open = false;
    }
    if download {
        start_download(app);
    }
}

fn start_download(app: &mut App) {
    let Some(release) = app.update.release.clone() else {
        return;
    };
    let Some(token) = app.token() else {
        app.update.error = Some("请先登录软件账号".into());
        return;
    };
    app.update.busy = true;
    app.update.progress = 0.0;
    app.update.error = None;
    app.spawn(move |tx| {
        let device_id = crate::soft::session::device_id();
        let result =
            crate::soft::update::download(&release, &token, &device_id, |done, total| {
                let _ = tx.send(Msg::UpdateProgress(done, total));
            });
        let _ = tx.send(Msg::UpdateDownloaded(result));
    });
}
