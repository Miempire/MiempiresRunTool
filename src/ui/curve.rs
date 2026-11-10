//! 可拖动控制点的曲线编辑器（配速 / 海拔）。
//!
//! 带坐标轴与刻度：横轴是完成进度（0~100%），纵轴是秒每公里或海拔米数。
//! 拖动圆点即可改变曲线；双击空白处加点，右键点删除。

use eframe::egui;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};

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
    pub selected: Option<usize>,
    drag: Option<usize>,
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
            selected: None,
            drag: None,
        }
    }

    pub fn default_pace(min_sec: f64, max_sec: f64) -> Self {
        Self {
            kind: CurveKind::Pace,
            points: default_pace(min_sec - 10.0, max_sec + 10.0),
            y_min: min_sec - 10.0,
            y_max: max_sec + 10.0,
            selected: None,
            drag: None,
        }
    }

    pub fn default_altitude(base: f64) -> Self {
        Self {
            kind: CurveKind::Altitude,
            points: default_altitude(base),
            y_min: base - 25.0,
            y_max: base + 25.0,
            selected: None,
            drag: None,
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
            Stroke::new(1.0, palette.border),
        );

        let plot = Rect::from_min_max(
            Pos2::new(outer.left() + 52.0, outer.top() + 10.0),
            Pos2::new(outer.right() - 12.0, outer.bottom() - 24.0),
        );
        if plot.width() <= 10.0 || plot.height() <= 10.0 {
            return false;
        }

        let to_screen = |x: f64, y: f64| -> Pos2 {
            let px = plot.left() + (x.clamp(0.0, 1.0) as f32) * plot.width();
            let ratio = ((y - self.y_min) / (self.y_max - self.y_min)).clamp(0.0, 1.0);
            let py = plot.bottom() - ratio as f32 * plot.height();
            Pos2::new(px, py)
        };
        let from_screen = |pos: Pos2| -> (f64, f64) {
            let x = ((pos.x - plot.left()) / plot.width()).clamp(0.0, 1.0) as f64;
            let ratio = ((plot.bottom() - pos.y) / plot.height()).clamp(0.0, 1.0) as f64;
            let y = self.y_min + ratio * (self.y_max - self.y_min);
            (x, y)
        };

        // 网格与坐标轴
        let grid = Stroke::new(1.0, palette.accent_soft);
        for step in 0..=4 {
            let x = step as f64 / 4.0;
            let top = to_screen(x, self.y_max);
            let bottom = to_screen(x, self.y_min);
            painter.line_segment([top, bottom], grid);
            painter.text(
                Pos2::new(bottom.x, plot.bottom() + 6.0),
                egui::Align2::CENTER_TOP,
                format!("{:.0}%", x * 100.0),
                egui::FontId::proportional(9.0),
                palette.text_dim,
            );
        }
        for step in 0..=4 {
            let ratio = step as f64 / 4.0;
            let y = self.y_min + ratio * (self.y_max - self.y_min);
            let left = to_screen(0.0, y);
            let right = to_screen(1.0, y);
            painter.line_segment([left, right], grid);
            painter.text(
                Pos2::new(plot.left() - 6.0, left.y),
                egui::Align2::RIGHT_CENTER,
                self.kind.y_text(y),
                egui::FontId::proportional(9.0),
                palette.text_dim,
            );
        }
        painter.line_segment(
            [plot.left_bottom(), plot.right_bottom()],
            Stroke::new(1.0, palette.border),
        );
        painter.line_segment(
            [plot.left_top(), plot.left_bottom()],
            Stroke::new(1.0, palette.border),
        );

        // 曲线
        let line: Vec<Pos2> = self
            .points
            .iter()
            .map(|(x, y)| to_screen(*x, *y))
            .collect();
        if line.len() >= 2 {
            painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0, palette.accent)));
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
        if down && self.drag.is_none() && response.is_pointer_button_down_on() {
            if let Some(index) = hit {
                self.drag = Some(index);
                self.selected = Some(index);
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
            painter.circle_filled(center, radius, palette.accent);
            painter.circle_stroke(center, radius, Stroke::new(1.5, Color32::WHITE));
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
                Stroke::new(1.0, palette.accent),
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
