//! 账号相关弹窗：软件账号登录/注册、账号与设置面板、官方账号登录。
//!
//! 登录态由 `soft::session` 加密落盘；这里只负责输入与展示。

use eframe::egui;
use egui::{Color32, RichText};

use super::{mobile, theme, App};
use crate::platform::InputKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    #[default]
    Login,
    Register,
}

#[derive(Default)]
pub struct AuthUi {
    pub open: bool,
    pub mode: AuthMode,
    pub username: String,
    pub password: String,
    pub nickname: String,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
}

#[derive(Default)]
pub struct OfficialUi {
    pub session: Option<crate::api::model::Session>,
    pub open: bool,
    pub username: String,
    pub password: String,
    pub busy: bool,
    pub error: Option<String>,
}

fn dim(text: &str, palette: &theme::Palette) -> RichText {
    RichText::new(text).size(12.0).color(palette.text_dim)
}

/// 软件账号登录 / 注册。
pub fn draw_soft_auth(app: &mut App, ctx: &egui::Context) {
    if !app.auth.open {
        return;
    }
    let palette = app.palette;
    let mut submit = false;
    let mut close = false;

    egui::Window::new("软件账号")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(330.0)
        .show(ctx, |ui| {
            ui.set_max_width(330.0);

            ui.horizontal(|ui| {
                for mode in [AuthMode::Login, AuthMode::Register] {
                    let active = app.auth.mode == mode;
                    let text = match mode {
                        AuthMode::Login => "登录",
                        AuthMode::Register => "注册",
                    };
                    let button = egui::Button::new(
                        RichText::new(text)
                            .size(14.0)
                            .color(if active { Color32::WHITE } else { palette.accent }),
                    )
                    .fill(if active { palette.accent } else { palette.accent_soft })
                    .rounding(egui::Rounding::same(12.0))
                    .min_size(egui::vec2(96.0, 36.0));
                    if ui.add(button).clicked() {
                        app.auth.mode = mode;
                        app.auth.error = None;
                        app.auth.notice = None;
                    }
                }
            });

            ui.add_space(8.0);
            ui.label(dim("账号", &palette));
            mobile::text_edit(
                ui,
                "soft_auth_user",
                &mut app.auth.username,
                InputKind::Text,
                300.0,
            );
            ui.add_space(4.0);
            ui.label(dim("密码", &palette));
            mobile::text_edit(
                ui,
                "soft_auth_pass",
                &mut app.auth.password,
                InputKind::Password,
                300.0,
            );

            if app.auth.mode == AuthMode::Register {
                ui.add_space(4.0);
                ui.label(dim("昵称（可选）", &palette));
                mobile::text_edit(
                    ui,
                    "soft_auth_nick",
                    &mut app.auth.nickname,
                    InputKind::Text,
                    300.0,
                );
            }

            if let Some(notice) = &app.auth.notice {
                ui.add_space(4.0);
                ui.label(RichText::new(notice).size(12.0).color(palette.ok));
            }
            if let Some(error) = &app.auth.error {
                ui.add_space(4.0);
                ui.label(RichText::new(error).size(12.0).color(palette.err));
            }

            ui.add_space(10.0);
            let ready = !app.auth.busy
                && !app.auth.username.trim().is_empty()
                && app.auth.password.chars().count() >= 6;
            let label = match app.auth.mode {
                AuthMode::Login if app.auth.busy => "登录中…",
                AuthMode::Register if app.auth.busy => "提交中…",
                AuthMode::Login => "登录",
                AuthMode::Register => "注册",
            };
            let width = ui.available_width();
            let button = egui::Button::new(RichText::new(label).color(Color32::WHITE).size(16.0))
                .fill(palette.accent)
                .rounding(egui::Rounding::same(14.0))
                .min_size(egui::vec2(width, 44.0));
            if ui.add_enabled(ready, button).clicked() {
                submit = true;
            }

            ui.add_space(4.0);
            if theme::ghost_button(ui, &palette, "取消").clicked() {
                close = true;
            }
            ui.add_space(2.0);
            ui.label(dim("密码至少 6 位；账号需管理员开通后使用。", &palette));
        });

    if close {
        app.auth.open = false;
    }
    if submit {
        let username = app.auth.username.trim().to_string();
        let password = app.auth.password.clone();
        let nickname = app.auth.nickname.trim().to_string();
        match app.auth.mode {
            AuthMode::Login => app.login(username, password),
            AuthMode::Register => app.register(username, password, nickname),
        }
    }
}

/// 账号与设置面板（含官方账号、主题、服务器、更新、退出）。
pub fn draw_account_sheet(app: &mut App, ctx: &egui::Context) {
    draw_official(app, ctx);

    if !app.account_open {
        return;
    }
    let palette = app.palette;
    let mut close = false;
    let mut refresh = false;
    let mut check_update = false;
    let mut open_server = false;
    let mut open_about = false;
    let mut open_official = false;
    let mut logout = false;
    let mut pick_theme: Option<super::theme::ThemeKind> = None;
    let mut notice_ack = false;

    let max_height = (ctx.screen_rect().height() * 0.78).max(320.0);

    egui::Window::new("账号与设置")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(340.0)
        .show(ctx, |ui| {
            ui.set_max_width(340.0);
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // 首次设备信息说明。
                    if !app.settings.device_notice_shown {
                        theme::card_frame(&palette).show(ui, |ui| {
                            ui.label(
                                RichText::new("设备信息说明")
                                    .size(13.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.label(
                                RichText::new(
                                    "软件会静默读取本机品牌、型号与电量，用于生成设备信息与兼容性校验；\
                                     不会读取通讯录、短信等隐私内容。",
                                )
                                .size(11.5)
                                .color(palette.text_dim),
                            );
                            if theme::ghost_button(ui, &palette, "知道了").clicked() {
                                notice_ack = true;
                            }
                        });
                        ui.add_space(6.0);
                    }

                    // 软件账号。
                    theme::card_frame(&palette).show(ui, |ui| {
                        match app.session.as_ref() {
                            Some(session) => {
                                ui.label(
                                    RichText::new(session.display_name())
                                        .size(17.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.label(
                                    RichText::new(session.status_text())
                                        .size(12.5)
                                        .color(if session.is_active() {
                                            palette.ok
                                        } else {
                                            palette.err
                                        }),
                                );
                                if let Some(expire) = &session.expires_on {
                                    ui.label(dim(
                                        &format!(
                                            "到期：{}",
                                            expire.chars().take(10).collect::<String>()
                                        ),
                                        &palette,
                                    ));
                                }
                                ui.label(dim(&format!("账号：{}", session.username), &palette));
                            }
                            None => {
                                ui.label(
                                    RichText::new("未登录")
                                        .size(16.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.label(dim("登录后即可使用路网与跑步功能。", &palette));
                            }
                        }
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            if app.session.is_some()
                                && theme::ghost_button(ui, &palette, "刷新授权").clicked()
                            {
                                refresh = true;
                            }
                            if theme::ghost_button(ui, &palette, "检查更新").clicked() {
                                check_update = true;
                            }
                        });
                    });

                    ui.add_space(6.0);
                    // 官方账号。
                    theme::card_frame(&palette).show(ui, |ui| {
                        ui.label(
                            RichText::new("官方运动账号")
                                .size(13.0)
                                .strong()
                                .color(palette.text),
                        );
                        match app.official.session.as_ref() {
                            Some(session) => {
                                ui.label(
                                    RichText::new(format!(
                                        "已登录 · {}",
                                        if session.name.trim().is_empty() {
                                            format!("uid {}", session.uid)
                                        } else {
                                            session.name.clone()
                                        }
                                    ))
                                    .size(12.0)
                                    .color(palette.ok),
                                );
                                ui.horizontal(|ui| {
                                    if theme::ghost_button(ui, &palette, "切换账号").clicked() {
                                        open_official = true;
                                    }
                                    if theme::ghost_button(ui, &palette, "退出登录").clicked() {
                                        app.official_logout();
                                    }
                                });
                            }
                            None => {
                                ui.label(dim("未登录（跑步提交与运动记录需要）", &palette));
                                if theme::ghost_button(ui, &palette, "登录官方账号").clicked() {
                                    open_official = true;
                                }
                            }
                        }
                    });

                    ui.add_space(6.0);
                    // 外观。
                    theme::card_frame(&palette).show(ui, |ui| {
                        ui.label(
                            RichText::new("外观")
                                .size(13.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.horizontal(|ui| {
                            for kind in [
                                super::theme::ThemeKind::Sakura,
                                super::theme::ThemeKind::Ocean,
                            ] {
                                let active = app.theme == kind;
                                let button = egui::Button::new(
                                    RichText::new(kind.label()).size(13.0).color(if active {
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
                                .rounding(egui::Rounding::same(12.0))
                                .min_size(egui::vec2(88.0, 34.0));
                                if ui.add(button).clicked() {
                                    pick_theme = Some(kind);
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                    // 服务器与关于。
                    theme::card_frame(&palette).show(ui, |ui| {
                        ui.label(
                            RichText::new("服务器")
                                .size(13.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.label(dim(&crate::config::base_url(), &palette));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if theme::ghost_button(ui, &palette, "更换地址").clicked() {
                                open_server = true;
                            }
                            if theme::ghost_button(ui, &palette, "关于").clicked() {
                                open_about = true;
                            }
                        });
                    });

                    ui.add_space(10.0);
                    if app.session.is_some() {
                        let width = ui.available_width();
                        let button = egui::Button::new(
                            RichText::new("退出登录").color(palette.err).size(15.0),
                        )
                        .fill(palette.card)
                        .stroke(egui::Stroke::new(1.0_f32, palette.border))
                        .rounding(egui::Rounding::same(14.0));
                        if ui.add_sized([width, 42.0], button).clicked() {
                            logout = true;
                        }
                    }
                    ui.add_space(6.0);
                    if theme::ghost_button(ui, &palette, "关闭").clicked() {
                        close = true;
                    }
                });
        });

    if let Some(kind) = pick_theme {
        app.theme = kind;
    }
    if notice_ack {
        app.settings.device_notice_shown = true;
        let _ = crate::config::save(&app.settings);
    }
    if refresh {
        app.refresh_license();
    }
    if check_update {
        app.check_update(true);
    }
    if open_server {
        app.server_input = crate::config::base_url();
        app.server_error = None;
        app.server_open = true;
    }
    if open_about {
        app.about_open = true;
    }
    if open_official {
        app.official.open = true;
        app.official.error = None;
    }
    if logout {
        app.logout();
        close = true;
    }
    if close {
        app.account_open = false;
    }
}

/// 官方账号登录弹窗（跑步前自动弹出）。
fn draw_official(app: &mut App, ctx: &egui::Context) {
    if !app.official.open {
        return;
    }
    let palette = app.palette;
    let mut submit = false;
    let mut close = false;

    egui::Window::new("官方账号登录")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(330.0)
        .show(ctx, |ui| {
            ui.set_max_width(330.0);
            ui.label(dim(
                "使用学校运动平台的账号密码。登录状态会加密保存在本机。",
                &palette,
            ));
            ui.add_space(6.0);
            ui.label(dim("账号", &palette));
            mobile::text_edit(
                ui,
                "official_user",
                &mut app.official.username,
                InputKind::Text,
                300.0,
            );
            ui.add_space(4.0);
            ui.label(dim("密码", &palette));
            mobile::text_edit(
                ui,
                "official_pass",
                &mut app.official.password,
                InputKind::Password,
                300.0,
            );
            if let Some(error) = &app.official.error {
                ui.add_space(4.0);
                ui.label(RichText::new(error).size(12.0).color(palette.err));
            }
            ui.add_space(10.0);
            let ready = !app.official.busy
                && !app.official.username.trim().is_empty()
                && !app.official.password.is_empty();
            let label = if app.official.busy {
                "登录中…"
            } else {
                "登录"
            };
            let width = ui.available_width();
            let button = egui::Button::new(RichText::new(label).color(Color32::WHITE).size(16.0))
                .fill(palette.accent)
                .rounding(egui::Rounding::same(14.0))
                .min_size(egui::vec2(width, 44.0));
            if ui.add_enabled(ready, button).clicked() {
                submit = true;
            }
            ui.add_space(4.0);
            if theme::ghost_button(ui, &palette, "取消").clicked() {
                close = true;
            }
        });

    if close {
        app.official.open = false;
    }
    if submit {
        let username = app.official.username.trim().to_string();
        let password = app.official.password.clone();
        app.official_login(username, password);
    }
}
