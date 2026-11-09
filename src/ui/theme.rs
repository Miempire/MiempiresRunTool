//! 极简二次元双主题：粉色（Sakura）/ 蓝色（Ocean）。

use eframe::egui::{self, Color32, Rounding, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Sakura,
    Ocean,
}

impl ThemeKind {
    pub fn parse(value: &str) -> Self {
        if value.eq_ignore_ascii_case("ocean") {
            Self::Ocean
        } else {
            Self::Sakura
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sakura => "sakura",
            Self::Ocean => "ocean",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sakura => "粉色",
            Self::Ocean => "蓝色",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Self::Sakura => Palette {
                bg: Color32::from_rgb(255, 247, 250),
                card: Color32::from_rgb(255, 255, 255),
                accent: Color32::from_rgb(255, 143, 177),
                accent_soft: Color32::from_rgb(255, 226, 234),
                text: Color32::from_rgb(61, 43, 51),
                text_dim: Color32::from_rgb(154, 124, 136),
                border: Color32::from_rgb(242, 215, 225),
                ok: Color32::from_rgb(86, 181, 138),
                warn: Color32::from_rgb(232, 165, 84),
                err: Color32::from_rgb(224, 96, 112),
                map_road: Color32::from_rgb(240, 214, 223),
                map_outline: Color32::from_rgb(255, 143, 177),
                map_vertex: Color32::from_rgb(255, 255, 255),
                map_building: Color32::from_rgb(248, 238, 242),
            },
            Self::Ocean => Palette {
                bg: Color32::from_rgb(244, 249, 255),
                card: Color32::from_rgb(255, 255, 255),
                accent: Color32::from_rgb(107, 182, 255),
                accent_soft: Color32::from_rgb(222, 239, 255),
                text: Color32::from_rgb(34, 48, 61),
                text_dim: Color32::from_rgb(125, 148, 168),
                border: Color32::from_rgb(216, 232, 247),
                ok: Color32::from_rgb(78, 175, 132),
                warn: Color32::from_rgb(224, 161, 74),
                err: Color32::from_rgb(214, 88, 104),
                map_road: Color32::from_rgb(214, 230, 245),
                map_outline: Color32::from_rgb(107, 182, 255),
                map_vertex: Color32::from_rgb(255, 255, 255),
                map_building: Color32::from_rgb(238, 246, 253),
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
