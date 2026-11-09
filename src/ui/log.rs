//! 运行日志：彩色滚动文本，自动滚底，上限 2000 行。
//! 入站行的 √/×/⚠ 前缀仅用于着色，展示时剥离。

use super::theme::Palette;
use crate::textlog::{classify, LogKind};
use egui::{Color32, RichText, ScrollArea};
use std::collections::VecDeque;

const MAX_LINES: usize = 2000;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub text: String,
    pub kind: LogKind,
}

impl LogLine {
    fn color(&self, palette: &Palette) -> Color32 {
        match self.kind {
            LogKind::Plain => palette.text,
            LogKind::Ok => palette.ok,
            LogKind::Err => palette.err,
            LogKind::Warn => palette.warn,
        }
    }
}

#[derive(Default)]
pub struct LogStore {
    lines: VecDeque<LogLine>,
}

impl LogStore {
    pub fn push(&mut self, msg: &str) {
        for line in msg.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if self.lines.len() >= MAX_LINES {
                self.lines.pop_front();
            }
            let (kind, text) = classify(t);
            self.lines.push_back(LogLine {
                text: text.to_string(),
                kind,
            });
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn last(&self) -> Option<&str> {
        self.lines.back().map(|line| line.text.as_str())
    }

    pub fn render(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.lines {
                    ui.label(
                        RichText::new(&line.text)
                            .monospace()
                            .size(12.5)
                            .color(line.color(palette)),
                    );
                }
            });
    }
}
