//! Miempire's RUN Tool —— 安卓专用极简客户端。
//!
//! 账号与授权、路网分发、更新、日志均由我方服务器提供；
//! 官方跑步协议（登录/轨迹/提交/极验）沿用原实现，算法不做改动。

mod api;
mod config;
mod crypto;
mod log_upload;
mod road_editor;
mod secure_store;
mod soft;
mod textlog;
mod track;
mod ui;

pub mod battery;
pub mod geetoken;
pub mod location;
pub mod platform;
pub mod sensors;

#[cfg(all(target_os = "android", feature = "android"))]
mod android;

#[cfg(feature = "gui")]
pub fn run_gui(options: eframe::NativeOptions) -> eframe::Result<()> {
    eframe::run_native(
        "Miempire's RUN Tool",
        options,
        Box::new(|cc| {
            #[cfg(target_os = "android")]
            android::set_context(&cc.egui_ctx);
            Ok(Box::new(ui::App::new(cc)))
        }),
    )
}
