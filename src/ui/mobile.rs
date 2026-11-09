//! 手机端布局与输入 helpers（Android 使用原生输入框，避免软键盘遮挡）。

use eframe::egui;
use std::hash::Hash;

use crate::platform::InputKind;

pub const COMPACT_BREAKPOINT: f32 = 620.0;
pub const TOUCH_HEIGHT: f32 = 44.0;

pub fn is_compact(width: f32) -> bool {
    width <= COMPACT_BREAKPOINT
}

pub fn compact_ui(ui: &egui::Ui) -> bool {
    is_compact(ui.available_width())
}

pub fn row<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    if compact_ui(ui) {
        ui.horizontal_wrapped(add_contents)
    } else {
        ui.horizontal(add_contents)
    }
}

#[cfg(target_os = "android")]
fn input_width(ui: &egui::Ui, desired: f32) -> f32 {
    desired.min(ui.available_width().max(80.0))
}

#[cfg(target_os = "android")]
fn native_text_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut String,
    kind: InputKind,
    desired_width: f32,
) -> egui::Response {
    if let Some(edited) = crate::platform::take_edited_text(id.value()) {
        *value = edited;
    }
    let shown = match kind {
        InputKind::Password if value.is_empty() => "点击输入密码".to_owned(),
        InputKind::Password => "●".repeat(value.chars().count().min(16)),
        _ if value.is_empty() => "点击输入".to_owned(),
        _ => value.clone(),
    };
    let response = ui.add_sized(
        [input_width(ui, desired_width), TOUCH_HEIGHT],
        egui::Button::new(shown),
    );
    if response.clicked() {
        crate::platform::edit_text(id.value(), value, kind);
    }
    response
}

pub fn text_edit(
    ui: &mut egui::Ui,
    id_source: impl Hash,
    value: &mut String,
    kind: InputKind,
    desired_width: f32,
) -> egui::Response {
    let id = ui.make_persistent_id(id_source);
    #[cfg(target_os = "android")]
    {
        native_text_button(ui, id, value, kind, desired_width)
    }
    #[cfg(not(target_os = "android"))]
    {
        let mut edit = egui::TextEdit::singleline(value)
            .id(id)
            .desired_width(desired_width);
        if matches!(kind, InputKind::Password) {
            edit = edit.password(true);
        }
        ui.add(edit)
    }
}

/// 底部两个标签：跑步 / 运动记录。
pub fn tab_bar(
    ui: &mut egui::Ui,
    palette: &super::theme::Palette,
    labels: &[&str],
    selected: &mut usize,
) -> Vec<egui::Response> {
    let width = ui.available_width();
    let count = labels.len().max(1) as f32;
    let spacing = 8.0;
    let item_width = ((width - spacing * (count - 1.0)) / count).max(60.0);
    let mut responses = Vec::with_capacity(labels.len());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = spacing;
        for (index, label) in labels.iter().enumerate() {
            let active = *selected == index;
            let button = egui::Button::new(
                egui::RichText::new(*label)
                    .size(15.0)
                    .color(if active { palette.accent } else { palette.text_dim }),
            )
            .fill(if active { palette.accent_soft } else { palette.card })
            .stroke(egui::Stroke::new(
                1.0,
                if active { palette.accent } else { palette.border },
            ))
            .rounding(egui::Rounding::same(14.0))
            .min_size(egui::vec2(item_width, TOUCH_HEIGHT));
            let response = ui.add(button);
            if response.clicked() {
                *selected = index;
            }
            responses.push(response);
        }
    });
    responses
}
