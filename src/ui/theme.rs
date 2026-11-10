//! 极简二次元双主题：蓝紫（默认，带渐变主按钮）/ 纯白（跑步弹窗内切换）。
//!
//! Palette 存的是离散颜色，渐变主按钮由 `gradient_button` 用 painter 绘制。

use eframe::egui::{self, Color32, Rounding, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// 蓝紫渐变主色（全局默认）。
    Violet,
    /// 跑界面纯白底，强调色仍保留蓝紫。
    RunWhite,
}

impl ThemeKind {
    pub fn parse(value: &str) -> Self {
        match value {
            "ocean" | "violet" => Self::Violet,
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
                grad_from: Color32::from_rgb(120, 98, 235),
                grad_to: Color32::from_rgb(96, 165, 250),
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
                grad_from: Color32::from_rgb(120, 98, 235),
                grad_to: Color32::from_rgb(96, 165, 250),
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
    /// 主按钮渐变起始色（左上）。
    pub grad_from: Color32,
    /// 主按钮渐变结束色（右下）。
    pub grad_to: Color32,
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
        visuals.hyperlink_color = self.accent;
        visuals.widgets.noninteractive.bg_fill = self.card;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_dim);
        visuals.widgets.noninteractive.rounding = Rounding::same(14.0);
        visuals.widgets.inactive.bg_fill = self.accent_soft;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.inactive.rounding = Rounding::same(14.0);
        visuals.widgets.hovered.bg_fill = self.accent_soft;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.hovered.rounding = Rounding::same(14.0);
        visuals.widgets.active.bg_fill = self.accent;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.active.rounding = Rounding::same(14.0);
        // window_shadow 和 spacing 在 0.29 改了，用默认值先编译过。
        style.visuals = visuals;

        use egui::{FontFamily::Proportional, FontId, TextStyle};
        style.text_styles.insert(TextStyle::Heading, FontId::new(20.0, Proportional));
        style.text_styles.insert(TextStyle::Body, FontId::new(15.0, Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(15.0, Proportional));
        style.text_styles.insert(TextStyle::Small, FontId::new(12.0, Proportional));
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(16.0, 10.0);
        ctx.set_style(style);
    }
}

/// 白色圆角卡片。
pub fn card_frame(palette: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.border))
        .rounding(Rounding::same(18.0))
        .inner_margin(egui::Margin::same(14.0))
}

/// 蓝紫渐变主按钮（带投影与 hover 提亮）。
pub fn primary_button(ui: &mut egui::Ui, palette: &Palette, text: &str) -> egui::Response {
    let desired = egui::vec2(ui.available_width().min(220.0).max(140.0), 46.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let painter = ui.painter_at(rect);
    draw_gradient_button(&painter, rect, palette, text, response.hovered(), response.dragged());
    response
}

/// 蓝紫渐变全宽按钮。
pub fn primary_button_wide(ui: &mut egui::Ui, palette: &Palette, text: &str) -> egui::Response {
    let desired = egui::vec2(ui.available_width().max(140.0), 46.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let painter = ui.painter_at(rect);
    draw_gradient_button(&painter, rect, palette, text, response.hovered(), response.dragged());
    response
}

fn draw_gradient_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    palette: &Palette,
    text: &str,
    hovered: bool,
    dragged: bool,
) {
    let rounding = Rounding::same(14.0);
    // 纵向渐变填充
    let from = palette.grad_from;
    let to = palette.grad_to;
    let steps = 16;
    let height = rect.height();
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let color = mix_color(from, to, t);
        let y0 = rect.top() + t * height;
        let y1 = rect.top() + (t + 1.0 / steps as f32) * height;
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            ),
            rounding,
            color,
        );
    }
    // hover / pressed 反馈
    if dragged {
        painter.rect_filled(
            rect,
            rounding,
            Color32::from_rgba_unmultiplied(0, 0, 0, 40),
        );
    } else if hovered {
        painter.rect_filled(
            rect,
            rounding,
            Color32::from_rgba_unmultiplied(255, 255, 255, 28),
        );
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(16.0),
        Color32::WHITE,
    );
}

fn mix_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let a = a.to_array();
    let b = b.to_array();
    Color32::from_rgba_unmultiplied(
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
        lerp(a[3], b[3], t),
    )
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

/// 次要描边按钮。
pub fn ghost_button(ui: &mut egui::Ui, palette: &Palette, text: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(text).color(palette.accent).size(15.0))
        .fill(palette.card)
        .stroke(Stroke::new(1.2, palette.border))
        .rounding(Rounding::same(14.0))
        .min_size(egui::vec2(0.0, 42.0));
    ui.add(button)
}

/// 跑步弹窗切换纯白主题：返回临时替换后的 palette，退出弹窗恢复。
pub fn run_white_palette(global: &Palette) -> Palette {
    // 用 RunWhite palette 的 bg/card/border，但 accent 保留全局蓝紫渐变。
    let white = ThemeKind::RunWhite.palette();
    Palette {
        bg: white.bg,
        card: white.card,
        accent: global.accent,
        accent_soft: global.accent_soft,
        text: global.text,
        text_dim: white.text_dim,
        border: white.border,
        ok: global.ok,
        warn: global.warn,
        err: global.err,
        map_road: white.map_road,
        map_outline: global.accent,
        map_vertex: global.map_vertex,
        map_building: white.map_building,
        grad_from: global.grad_from,
        grad_to: global.grad_to,
    }
}
