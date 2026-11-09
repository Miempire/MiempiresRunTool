/// 桌面端仅用于开发自测；正式产物为 Android（由 GitHub Actions 构建）。
#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 880.0])
            .with_min_inner_size([360.0, 640.0]),
        ..Default::default()
    };
    miempires_run_tool::run_gui(options)
}

#[cfg(target_os = "android")]
fn main() {}
