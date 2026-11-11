//! 可拖动控制点的曲线编辑器（配速 / 海拔）。
//!
//! 带坐标轴与刻度：横轴是完成进度（0~100%），纵轴是秒每公里或海拔米数。
//! 拖动圆点即可改变曲线；双击空白处加点，右键点删除。

use eframe::egui;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use rand::Rng;

use super::theme;
use crate::track::altitude::{sample_curve, validate_curve};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CurveKind {
    Pace,
    Altitude,
}

impl CurveKind {
    pub fn title(self) -> &'static str {
        match self {
            CurveKind::Pace => "配速曲线",
            CurveKind::Altitude => "海拔曲线",
        }
    }

    pub fn y_text(self, value: f64) -> String {
        match self {
            CurveKind::Pace => {
                let total = value.round() as i64;
                format!("{}'{:02}\"", total / 60, total % 60)
            }
            CurveKind::Altitude => format!("{:.0} m", value),
        }
    }

    pub fn range(self) -> (f64, f64) {
        match self {
            CurveKind::Pace => (60.0, 3600.0),
            CurveKind::Altitude => (-500.0, 9000.0),
        }
    }
}

pub struct CurveEditor {
    pub kind: CurveKind,
    /// 控制点：x 为进度（0~1，递增），y 为数值。
    pub points: Vec<(f64, f64)>,
    pub y_min: f64,
    pub y_max: f64,
    /// 噪声幅度（配速：秒/公里；海拔：米）。
    pub noise_amp: f64,
    /// 噪声生成时铺的控制点个数（越少越好拖，越多起伏越细）。
    pub noise_points: usize,
    pub selected: Option<usize>,
    drag: Option<usize>,
    /// 纵轴视窗中心与跨度（放大/平移后的可见区间）。
    view_center: f64,
    view_span: f64,
    panning: bool,
}

impl CurveEditor {
    pub fn new(kind: CurveKind, points: Vec<(f64, f64)>, y_min: f64, y_max: f64) -> Self {
        let valid = points.len() >= 2
            && points.windows(2).all(|pair| pair[1].0 > pair[0].0)
            && points
                .iter()
                .all(|(x, y)| x.is_finite() && y.is_finite() && (0.0..=1.0).contains(x));
        let points = if valid {
            points
        } else {
            match kind {
                CurveKind::Pace => default_pace(y_min, y_max),
                CurveKind::Altitude => default_altitude((y_min + y_max) * 0.5),
            }
        };
        Self {
            kind,
            points,
            y_min,
            y_max,
            noise_amp: default_noise(kind),
            noise_points: DEFAULT_NOISE_POINTS,
            selected: None,
            drag: None,
            view_center: (y_min + y_max) * 0.5,
            view_span: (y_max - y_min).max(1.0),
            panning: false,
        }
    }

    pub fn default_pace(min_sec: f64, max_sec: f64) -> Self {
        Self {
            kind: CurveKind::Pace,
            points: default_pace(min_sec - 10.0, max_sec + 10.0),
            y_min: min_sec - 10.0,
            y_max: max_sec + 10.0,
            noise_amp: default_noise(CurveKind::Pace),
            noise_points: DEFAULT_NOISE_POINTS,
            selected: None,
            drag: None,
            view_center: (min_sec + max_sec) * 0.5,
            view_span: (max_sec - min_sec + 20.0).max(20.0),
            panning: false,
        }
    }

    pub fn default_altitude(base: f64) -> Self {
        Self {
            kind: CurveKind::Altitude,
            points: default_altitude(base),
            y_min: base - 25.0,
            y_max: base + 25.0,
            noise_amp: default_noise(CurveKind::Altitude),
            noise_points: DEFAULT_NOISE_POINTS,
            selected: None,
            drag: None,
            view_center: base,
            view_span: 50.0,
            panning: false,
        }
    }

    /// 重置为默认形状（保留当前坐标范围）。
    pub fn reset(&mut self) {
        self.points = match self.kind {
            CurveKind::Pace => default_pace(self.y_min, self.y_max),
            CurveKind::Altitude => default_altitude((self.y_min + self.y_max) * 0.5),
        };
        self.selected = None;
        self.drag = None;
        self.reset_view();
    }

    /// 调整纵轴范围，并把越界的控制点夹回范围内。
    pub fn set_range(&mut self, y_min: f64, y_max: f64) {
        let (low, high) = if y_min < y_max {
            (y_min, y_max)
        } else {
            (y_max, y_min)
        };
        if (low - self.y_min).abs() < f64::EPSILON && (high - self.y_max).abs() < f64::EPSILON {
            return;
        }
        self.y_min = low;
        self.y_max = high;
        for point in &mut self.points {
            point.1 = point.1.clamp(low, high);
        }
        self.reset_view();
    }

    /// 整体平移曲线（用于跟随海拔基准变化）。
    pub fn shift(&mut self, delta: f64) {
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        for point in &mut self.points {
            point.1 += delta;
        }
    }

    fn full_span(&self) -> f64 {
        (self.y_max - self.y_min).max(1.0)
    }

    pub fn reset_view(&mut self) {
        self.view_center = (self.y_min + self.y_max) * 0.5;
        self.view_span = self.full_span();
        self.panning = false;
    }

    /// 当前可见纵轴区间。
    pub fn view_range(&self) -> (f64, f64) {
        let half = self.view_span * 0.5;
        (self.view_center - half, self.view_center + half)
    }

    pub fn is_zoomed(&self) -> bool {
        self.view_span < self.full_span() - 1e-9
    }

    fn clamp_view(&mut self) {
        let full = self.full_span();
        self.view_span = self.view_span.clamp(full * 0.05, full);
        let half = self.view_span * 0.5;
        let low = self.y_min + half;
        let high = self.y_max - half;
        self.view_center = if low <= high {
            self.view_center.clamp(low, high)
        } else {
            (self.y_min + self.y_max) * 0.5
        };
    }

    /// 以屏幕纵向比例（0 = 底部）为锚点缩放，factor < 1 为放大。
    fn zoom_at(&mut self, anchor_ratio: f64, factor: f64) {
        let full = self.full_span();
        let (vmin, vmax) = self.view_range();
        let anchor = vmin + anchor_ratio * (vmax - vmin);
        self.view_span = (self.view_span * factor).clamp(full * 0.05, full);
        self.view_center = anchor + (0.5 - anchor_ratio) * self.view_span;
        self.clamp_view();
    }

    /// 在原曲线上叠加多频起伏，生成可直接拖动的控制点。
    /// 配速单位是秒/公里，海拔单位是米，幅度由 `noise_amp` 控制。
    pub fn apply_noise(&mut self) {
        let amp = self.noise_amp.max(0.0);
        if amp <= 0.0 || self.points.len() < 2 {
            return;
        }
        let (kind_min, kind_max) = self.kind.range();
        let count = self.noise_points.clamp(5, 64);
        let mut rng = rand::thread_rng();
        let base: Vec<(f64, f64)> = (0..count)
            .map(|index| {
                let x = index as f64 / (count - 1) as f64;
                (x, sample_curve(&self.points, x))
            })
            .collect();
        let phases: [f64; 3] = [
            rng.gen_range(0.0..std::f64::consts::TAU),
            rng.gen_range(0.0..std::f64::consts::TAU),
            rng.gen_range(0.0..std::f64::consts::TAU),
        ];
        let freqs: [f64; 3] = [
            rng.gen_range(0.8..1.6),
            rng.gen_range(2.2..3.6),
            rng.gen_range(5.0..8.0),
        ];
        let mut out = Vec::with_capacity(count);
        for (x, y0) in &base {
            let tau = std::f64::consts::TAU;
            let wave = 0.55 * (tau * freqs[0] * x + phases[0]).sin()
                + 0.28 * (tau * freqs[1] * x + phases[1]).sin()
                + 0.17 * (tau * freqs[2] * x + phases[2]).sin();
            let jitter = rng.gen_range(-0.2..0.2);
            let y = (y0 + amp * (wave + jitter))
                .clamp(self.y_min, self.y_max)
                .clamp(kind_min, kind_max);
            out.push((*x, y));
        }
        // 首尾保持原值：闭合路线的海拔要回到同一高度。
        out[0].1 = base[0].1;
        let last = count - 1;
        out[last].1 = base[last].1;
        self.points = out;
        self.selected = None;
        self.drag = None;
        self.panning = false;
    }

    /// 控制点沿进度均匀排布（保留纵值顺序），便于噪声后整理点位。
    pub fn redistribute(&mut self) {
        let count = self.points.len();
        if count < 2 {
            return;
        }
        for (index, point) in self.points.iter_mut().enumerate() {
            point.0 = index as f64 / (count - 1) as f64;
        }
        self.selected = None;
        self.drag = None;
    }

    pub fn value_at(&self, x: f64) -> f64 {
        sample_curve(&self.points, x)
    }

    pub fn sample(&self) -> Vec<(f64, f64)> {
        self.points.clone()
    }

    pub fn validate(&self) -> Result<(), String> {
        let (min_y, max_y) = self.kind.range();
        validate_curve(&self.points, min_y, max_y, self.kind.title())
    }

    /// 绘制编辑器；返回 true 表示曲线被修改。
    pub fn draw(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) -> bool {
        let height = 150.0;
        let width = ui.available_width().max(220.0);
        let (response, painter) =
            ui.allocate_painter(Vec2::new(width, height), Sense::click_and_drag());
        let outer = response.rect;
        painter.rect_filled(outer, egui::Rounding::same(8.0), palette.card);
        painter.rect_stroke(
            outer,
            egui::Rounding::same(8.0),
            Stroke::new(1.0_f32, palette.border),
        );

        let plot = Rect::from_min_max(
            Pos2::new(outer.left() + 52.0, outer.top() + 10.0),
            Pos2::new(outer.right() - 12.0, outer.bottom() - 24.0),
        );
        if plot.width() <= 10.0 || plot.height() <= 10.0 {
            return false;
        }

        let (view_min, view_max) = self.view_range();
        let view_span = (view_max - view_min).max(1e-6);
        let to_screen = |x: f64, y: f64| -> Pos2 {
            let px = plot.left() + (x.clamp(0.0, 1.0) as f32) * plot.width();
            let ratio = ((y - view_min) / view_span) as f32;
            let py = plot.bottom() - ratio * plot.height();
            Pos2::new(px, py)
        };
        let from_screen = |pos: Pos2| -> (f64, f64) {
            let x = ((pos.x - plot.left()) / plot.width()).clamp(0.0, 1.0) as f64;
            let ratio = ((plot.bottom() - pos.y) / plot.height()) as f64;
            let y = view_min + ratio * view_span;
            (x, y)
        };

        // 滚轮缩放纵轴（以指针位置为锚点）。
        if response.hovered() {
            let scroll = ui.input(|input| input.raw_scroll_delta.y);
            if scroll.abs() > 0.5 {
                if let Some(pos) = response.hover_pos() {
                    if plot.contains(pos) {
                        let ratio =
                            ((plot.bottom() - pos.y) / plot.height()).clamp(0.0, 1.0) as f64;
                        let factor = if scroll > 0.0 { 0.88 } else { 1.0 / 0.88 };
                        self.zoom_at(ratio, factor);
                    }
                }
            }
        }

        // 网格与坐标轴
        let grid = Stroke::new(1.0_f32, palette.accent_soft);
        for step in 0..=4 {
            let x = step as f64 / 4.0;
            let px = plot.left() + x as f32 * plot.width();
            painter.line_segment(
                [Pos2::new(px, plot.top()), Pos2::new(px, plot.bottom())],
                grid,
            );
            painter.text(
                Pos2::new(px, plot.bottom() + 6.0),
                egui::Align2::CENTER_TOP,
                format!("{:.0}%", x * 100.0),
                egui::FontId::proportional(9.0),
                palette.text_dim,
            );
        }
        for step in 0..=4 {
            let ratio = step as f64 / 4.0;
            let y = view_min + ratio * view_span;
            let py = plot.bottom() - ratio as f32 * plot.height();
            painter.line_segment(
                [Pos2::new(plot.left(), py), Pos2::new(plot.right(), py)],
                grid,
            );
            painter.text(
                Pos2::new(plot.left() - 6.0, py),
                egui::Align2::RIGHT_CENTER,
                self.kind.y_text(y),
                egui::FontId::proportional(9.0),
                palette.text_dim,
            );
        }
        painter.line_segment(
            [plot.left_bottom(), plot.right_bottom()],
            Stroke::new(1.0_f32, palette.border),
        );
        painter.line_segment(
            [plot.left_top(), plot.left_bottom()],
            Stroke::new(1.0_f32, palette.border),
        );
        if self.is_zoomed() {
            let zoom = self.full_span() / self.view_span;
            painter.text(
                Pos2::new(plot.right() - 4.0, plot.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                format!("×{zoom:.1}"),
                egui::FontId::proportional(9.0),
                palette.text_dim,
            );
        }

        // 曲线（裁剪到绘图区，缩放后越界部分不画出来）
        let clipped = painter.with_clip_rect(plot);
        let line: Vec<Pos2> = self
            .points
            .iter()
            .map(|(x, y)| to_screen(*x, *y))
            .collect();
        if line.len() >= 2 {
            clipped.add(egui::Shape::line(
                line.clone(),
                Stroke::new(2.0_f32, palette.accent),
            ));
        }

        // 交互命中检测
        let pointer = response.hover_pos();
        let mut hit = pointer.and_then(|pos| {
            self.points
                .iter()
                .enumerate()
                .map(|(index, (x, y))| (index, to_screen(*x, *y).distance(pos)))
                .filter(|(_, distance)| *distance <= 14.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index)
        });
        if hit.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let mut changed = false;
        let down = ui.input(|input| input.pointer.primary_down());
        // 空白处拖动 = 平移纵轴视窗。
        if down && self.drag.is_none() && !self.panning && hit.is_none()
            && response.is_pointer_button_down_on()
        {
            if let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
                if plot.contains(pos) {
                    self.panning = true;
                }
            }
        }
        if self.panning {
            if !down {
                self.panning = false;
            } else {
                let delta = ui.input(|input| input.pointer.delta().y) as f64;
                if delta != 0.0 {
                    self.view_center += delta / plot.height() as f64 * self.view_span;
                    self.clamp_view();
                }
            }
        }
        if down && self.drag.is_none() && response.is_pointer_button_down_on() {
            if let Some(index) = hit {
                self.drag = Some(index);
                self.selected = Some(index);
                self.panning = false;
            }
        }
        if let Some(index) = self.drag {
            if !down {
                self.drag = None;
            } else if let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
                let (mut x, mut y) = from_screen(pos);
                y = y.clamp(self.y_min, self.y_max);
                let last = self.points.len() - 1;
                if index == 0 {
                    x = 0.0;
                } else if index == last {
                    x = 1.0;
                } else {
                    x = x.clamp(self.points[index - 1].0 + 0.01, self.points[index + 1].0 - 0.01);
                }
                if (self.points[index].0 - x).abs() > 1e-9
                    || (self.points[index].1 - y).abs() > 1e-9
                {
                    self.points[index] = (x, y);
                    changed = true;
                }
            }
        }

        // 双击空白处加点
        if response.double_clicked() && hit.is_none() && self.points.len() < 64 {
            if let Some(pos) = response.interact_pointer_pos() {
                if plot.contains(pos) {
                    let (x, y) = from_screen(pos);
                    let index = self
                        .points
                        .iter()
                        .position(|(px, _)| *px > x)
                        .unwrap_or(self.points.len());
                    if index > 0 && index < self.points.len() {
                        self.points.insert(index, (x, y.clamp(self.y_min, self.y_max)));
                        self.selected = Some(index);
                        changed = true;
                    }
                }
            }
        }
        // 右键删除中间点
        if response.secondary_clicked() && self.points.len() > 2 {
            if let Some(index) = hit {
                if index > 0 && index < self.points.len() - 1 {
                    self.points.remove(index);
                    self.selected = None;
                    self.drag = None;
                    hit = None;
                    changed = true;
                }
            }
        }

        // 控制点
        let last_index = self.points.len().saturating_sub(1);
        for (index, (x, y)) in self.points.iter().enumerate() {
            let center = to_screen(*x, *y);
            let active = self.selected == Some(index) || self.drag == Some(index);
            let radius = if active { 6.5 } else { 5.0 };
            clipped.circle_filled(center, radius, palette.accent);
            clipped.circle_stroke(center, radius, Stroke::new(1.5_f32, Color32::WHITE));
        }

        // 选中点的数值气泡
        let label_index = self.selected.or(hit).unwrap_or(usize::MAX);
        if label_index <= last_index {
            let (x, y) = self.points[label_index];
            let center = to_screen(x, y);
            let text = format!("{:.0}% · {}", x * 100.0, self.kind.y_text(y));
            let anchor = Pos2::new(center.x, center.y - 10.0);
            let width = 9.0 * text.chars().count() as f32 + 8.0;
            let bubble = Rect::from_center_size(
                Pos2::new(anchor.x, anchor.y - 8.0),
                Vec2::new(width, 16.0),
            );
            painter.rect_filled(bubble, egui::Rounding::same(4.0), palette.bg);
            painter.rect_stroke(
                bubble,
                egui::Rounding::same(4.0),
                Stroke::new(1.0_f32, palette.accent),
            );
            painter.text(
                bubble.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(9.5),
                palette.text,
            );
        }

        changed
    }
}

/// 噪声默认铺点个数。
const DEFAULT_NOISE_POINTS: usize = 33;

fn default_noise(kind: CurveKind) -> f64 {
    match kind {
        CurveKind::Pace => 8.0,
        CurveKind::Altitude => 3.0,
    }
}

fn default_pace(min_sec: f64, max_sec: f64) -> Vec<(f64, f64)> {
    let (low, high) = if min_sec < max_sec {
        (min_sec, max_sec)
    } else {
        (max_sec, min_sec)
    };
    let mid = (low + high) * 0.5;
    let fast = low + (high - low) * 0.25;
    let slow = low + (high - low) * 0.75;
    vec![
        (0.0, slow),
        (0.25, mid),
        (0.5, fast),
        (0.75, mid),
        (1.0, slow),
    ]
}

fn default_altitude(base: f64) -> Vec<(f64, f64)> {
    vec![
        (0.0, base),
        (0.25, base + 6.0),
        (0.5, base + 2.0),
        (0.75, base - 4.0),
        (1.0, base),
    ]
}

/// 参数页的小标题。
pub fn section_label(ui: &mut egui::Ui, palette: &theme::Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(text)
            .size(12.5)
            .strong()
            .color(palette.text_dim),
    );
}
