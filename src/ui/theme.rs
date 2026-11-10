//! 极简二次元双主题：蓝紫（默认）/ 纯白（跑步弹窗）。

use eframe::egui::{self, Color32, Rounding, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// 蓝紫（默认）。
    Violet,
    /// 纯白（跑步弹窗内使用）。
    RunWhite,
}

impl ThemeKind {
    pub fn parse(value: &str) -> Self {
        match value {
            "run_white" => Self::RunWhite,
            _ => Self::Violet,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Violet => "violet",
            Self::RunWhite => "run_white",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Violet => "蓝紫渐变",
            Self::RunWhite => "纯白（跑步弹窗）",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Self::Violet => Palette {
                bg: Color32::from_rgb(245, 243, 255),
                card: Color32::from_rgb(255, 255, 255),
                accent: Color32::from_rgb(120, 98, 235),
                accent_soft: Color32::from_rgb(234, 229, 255),
                text: Color32::from_rgb(38, 32, 58),
                text_dim: Color32::from_rgb(130, 118, 162),
                border: Color32::from_rgb(223, 216, 247),
                ok: Color32::from_rgb(86, 181, 138),
                warn: Color32::from_rgb(232, 165, 84),
                err: Color32::from_rgb(224, 96, 112),
                map_road: Color32::from_rgb(228, 222, 248),
                map_outline: Color32::from_rgb(120, 98, 235),
                map_vertex: Color32::from_rgb(255, 255, 255),
                map_building: Color32::from_rgb(244, 240, 252),
            },
            Self::RunWhite => Palette {
                bg: Color32::from_rgb(255, 255, 255),
                card: Color32::from_rgb(255, 255, 255),
                accent: Color32::from_rgb(120, 98, 235),
                accent_soft: Color32::from_rgb(240, 237, 255),
                text: Color32::from_rgb(38, 32, 58),
                text_dim: Color32::from_rgb(130, 118, 162),
                border: Color32::from_rgb(232, 228, 243),
                ok: Color32::from_rgb(86, 181, 138),
                warn: Color32::from_rgb(232, 165, 84),
                err: Color32::from_rgb(224, 96, 112),
                map_road: Color32::from_rgb(235, 230, 248),
                map_outline: Color32::from_rgb(120, 98, 235),
                map_vertex: Color32::from_rgb(255, 255, 255),
                map_building: Color32::from_rgb(248, 245, 253),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub card: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub border: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub map_road: Color32,
    pub map_outline: Color32,
    pub map_vertex: Color32,
    pub map_building: Color32,
}

impl Palette {
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let mut visuals = egui::Visuals::light();
        visuals.override_text_color = Some(self.text);
        visuals.panel_fill = self.bg;
        visuals.window_fill = self.card;
        visuals.extreme_bg_color = self.card;
        visuals.faint_bg_color = self.accent_soft;
        visuals.window_stroke = Stroke::new(1.0, self.border);
        visuals.selection.bg_fill = self.accent_soft;
        visuals.selection.stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.noninteractive.bg_fill = self.card;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_dim);
        visuals.widgets.noninteractive.rounding = Rounding::same(12.0);
        visuals.widgets.inactive.bg_fill = self.accent_soft;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.inactive.rounding = Rounding::same(12.0);
        visuals.widgets.hovered.bg_fill = self.accent_soft;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.hovered.rounding = Rounding::same(12.0);
        visuals.widgets.active.bg_fill = self.accent;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.active.rounding = Rounding::same(12.0);
        visuals.window_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 6.0),
            blur: 18.0,
            spread: 0.0,
            color: Color32::from_black_alpha(28),
        };
        style.visuals = visuals;

        use egui::{FontFamily::Proportional, FontId, TextStyle};
        style.text_styles.insert(TextStyle::Heading, FontId::new(20.0, Proportional));
        style.text_styles.insert(TextStyle::Body, FontId::new(15.0, Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(15.0, Proportional));
        style.text_styles.insert(TextStyle::Small, FontId::new(12.0, Proportional));
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 9.0);
        style.spacing.interact_size.y = 40.0;
        ctx.set_style(style);
    }
}

/// 白色圆角卡片。
pub fn card_frame(palette: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.border))
        .rounding(Rounding::same(16.0))
        .inner_margin(egui::Margin::same(14.0))
}

/// 主题色主按钮。
pub fn primary_button(ui: &mut egui::Ui, palette: &Palette, text: &str) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(text)
            .color(Color32::WHITE)
            .size(16.0),
    )
    .fill(palette.accent)
    .rounding(Rounding::same(14.0))
    .min_size(egui::vec2(0.0, 44.0));
    ui.add(button)
}

/// 次要按钮（描边）。
pub fn ghost_button(ui: &mut egui::Ui, palette: &Palette, text: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(text).color(palette.accent).size(15.0))
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.border))
        .rounding(Rounding::same(14.0))
        .min_size(egui::vec2(0.0, 40.0));
    ui.add(button)
}

/// 跑步弹窗切换纯白主题：bg/border 换成纯白底，强调色保留蓝紫。
pub fn run_white_palette(global: &Palette) -> Palette {
    Palette {
        bg: Color32::from_rgb(255, 255, 255),
        card: Color32::from_rgb(255, 255, 255),
        accent: global.accent,
        accent_soft: Color32::from_rgb(240, 237, 255),
        text: global.text,
        text_dim: global.text_dim,
        border: Color32::from_rgb(232, 228, 243),
        ok: global.ok,
        warn: global.warn,
        err: global.err,
        map_road: Color32::from_rgb(235, 230, 248),
        map_outline: global.accent,
        map_vertex: global.map_vertex,
        map_building: Color32::from_rgb(248, 245, 253),
    }
}
