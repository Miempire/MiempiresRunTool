//! 路网预览与轮廓编辑（跑道模式）。
//!
//! 默认轮廓来自 OSM 中的跑道候选；用户可拖动/增删顶点，所有顶点在编辑时
//! 吸附到最近的道路节点（默认 25m 内），保存前用与跑步相同的一套
//! `route_planner::validate_outline` 校验连通性与方向。

use route_planner::{Coord, RoadGraph};

use crate::track::geom::{MET_PER_DEG_LAT, MET_PER_DEG_LNG};

#[derive(Debug, Clone)]
pub struct RoadCandidate {
    pub key: u64,
    pub points: Vec<(f64, f64)>,
    pub length_m: f64,
}

#[derive(Debug, Clone)]
pub struct Preview {
    /// 道路（BD09，lat/lon）。
    pub edges: Vec<Vec<(f64, f64)>>,
    pub buildings: Vec<Vec<(f64, f64)>>,
    /// 当前轮廓（非闭合）。
    pub outline: Vec<(f64, f64)>,
    /// 可用跑道候选（按长度降序）。
    pub candidates: Vec<RoadCandidate>,
    /// 当前选中的候选下标（手动编辑后为 None）。
    pub selected: Option<usize>,
    pub source: &'static str,
}

pub fn distance_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dlat = (a.0 - b.0) * MET_PER_DEG_LAT;
    let dlon = (a.1 - b.1) * MET_PER_DEG_LNG;
    (dlat * dlat + dlon * dlon).sqrt()
}

/// 闭合轮廓周长（米）。
pub fn outline_length_m(points: &[(f64, f64)]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for index in 0..points.len() {
        total += distance_m(points[index], points[(index + 1) % points.len()]);
    }
    total
}

/// 构建预览：从**已对齐（BD09）**的路网中提取跑道候选。
///
/// 注意：调用方传入的路网必须已经过 `align_network`（`soft::roads::load_network`
/// 与跑步流程一致，都只对齐一次），此处不再重复对齐，否则坐标会被二次偏移。
pub fn build_preview(net: &RoadGraph, select: usize) -> Result<Preview, String> {
    let graph = net.clone();
    let mut candidates: Vec<RoadCandidate> = Vec::new();
    for candidate in route_planner::track_candidates(&graph) {
        if candidate.issue.is_some() {
            continue;
        }
        let points: Vec<(f64, f64)> = candidate.points.iter().map(|p| (p.lat, p.lon)).collect();
        if points.len() < 3 {
            continue;
        }
        candidates.push(RoadCandidate {
            key: candidate.key,
            points,
            length_m: candidate.length_m,
        });
    }
    candidates.sort_by(|a, b| a.length_m.partial_cmp(&b.length_m).unwrap_or(std::cmp::Ordering::Equal));
    candidates.reverse();
    let edges: Vec<Vec<(f64, f64)>> = graph
        .edges_deg()
        .iter()
        .map(|edge| edge.iter().map(|c| (c.lat, c.lon)).collect())
        .collect();
    let buildings = graph
        .buildings
        .iter()
        .map(|poly| poly.iter().map(|c| (c.lat, c.lon)).collect())
        .collect();
    if candidates.is_empty() {
        return Ok(Preview {
            edges,
            buildings,
            outline: Vec::new(),
            candidates,
            selected: None,
            source: "manual",
        });
    }
    let index = select.min(candidates.len() - 1);
    Ok(Preview {
        edges,
        buildings,
        outline: candidates[index].points.clone(),
        candidates,
        selected: Some(index),
        source: "candidate",
    })
}

/// 选择另一条候选跑道。
pub fn choose(preview: &mut Preview, index: usize) {
    if preview.candidates.is_empty() {
        return;
    }
    let index = index.min(preview.candidates.len() - 1);
    preview.outline = preview.candidates[index].points.clone();
    preview.selected = Some(index);
    preview.source = "candidate";
}

/// 把点吸附到最近道路节点（米制容差）。
pub fn snap(net: &RoadGraph, point: (f64, f64), tol_m: f64) -> Option<(f64, f64)> {
    let coord = Coord::new(point.1, point.0);
    route_planner::track::nearest_node(net, coord, tol_m).map(|node| {
        let c = net.node_coord(node);
        (c.lat, c.lon)
    })
}

/// 命中轮廓顶点（米制容差）。
pub fn hit_vertex(outline: &[(f64, f64)], point: (f64, f64), tol_m: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, vertex) in outline.iter().enumerate() {
        let d = distance_m(*vertex, point);
        if d <= tol_m && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((index, d));
        }
    }
    best.map(|(index, _)| index)
}

/// 命中轮廓线段，返回应插入的下标位置（在线段 i 与 i+1 之间插入）。
pub fn hit_segment(outline: &[(f64, f64)], point: (f64, f64), tol_m: f64) -> Option<usize> {
    if outline.len() < 2 {
        return None;
    }
    let mut best: Option<(usize, f64)> = None;
    for index in 0..outline.len() {
        let a = outline[index];
        let b = outline[(index + 1) % outline.len()];
        let d = point_segment_distance(point, a, b);
        if d <= tol_m && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((index, d));
        }
    }
    best.map(|(index, _)| index)
}

fn point_segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let ax = (a.1 - p.1) * MET_PER_DEG_LNG;
    let ay = (a.0 - p.0) * MET_PER_DEG_LAT;
    let bx = (b.1 - p.1) * MET_PER_DEG_LNG;
    let by = (b.0 - p.0) * MET_PER_DEG_LAT;
    let dx = bx - ax;
    let dy = by - ay;
    let length2 = dx * dx + dy * dy;
    if length2 <= 1e-9 {
        return (ax * ax + ay * ay).sqrt();
    }
    let t = (-(ax * dx + ay * dy) / length2).clamp(0.0, 1.0);
    let cx = ax + dx * t;
    let cy = ay + dy * t;
    (cx * cx + cy * cy).sqrt()
}

/// 插入顶点（`segment` 为 `hit_segment` 的返回值）。
pub fn insert_vertex(outline: &mut Vec<(f64, f64)>, segment: usize, point: (f64, f64)) {
    let index = (segment + 1).min(outline.len());
    outline.insert(index, point);
}

/// 删除顶点（不足 3 个时拒绝）。
pub fn remove_vertex(outline: &mut Vec<(f64, f64)>, index: usize) -> bool {
    if outline.len() <= 3 || index >= outline.len() {
        return false;
    }
    outline.remove(index);
    true
}

/// 平滑：每个顶点向其前后顶点中点靠拢 30%，保持形状但减少锯齿。
pub fn smooth(outline: &mut [(f64, f64)]) {
    let n = outline.len();
    if n < 4 {
        return;
    }
    let old = outline.to_vec();
    for index in 0..n {
        let prev = old[(index + n - 1) % n];
        let next = old[(index + 1) % n];
        let mid = ((prev.0 + next.0) / 2.0, (prev.1 + next.1) / 2.0);
        outline[index] = (
            old[index].0 * 0.7 + mid.0 * 0.3,
            old[index].1 * 0.7 + mid.1 * 0.3,
        );
    }
}

/// 反转方向（不影响闭合性，用于修正单向道路方向冲突）。
pub fn reverse(outline: &mut [(f64, f64)]) {
    outline.reverse();
}

/// 与跑步时一致的轮廓校验。
pub fn validate(net: &RoadGraph, outline: &[(f64, f64)]) -> Result<(), String> {
    if outline.len() < 3 {
        return Err("轮廓至少需要 3 个顶点".into());
    }
    route_planner::validate_outline(net, outline, 5.0).map(|_| ())
}

/// 把轮廓的全部顶点吸附到道路节点；返回吸附成功的数量。
pub fn snap_all(net: &RoadGraph, outline: &mut [(f64, f64)], tol_m: f64) -> usize {
    let mut snapped = 0;
    for vertex in outline.iter_mut() {
        if let Some(point) = snap(net, *vertex, tol_m) {
            *vertex = point;
            snapped += 1;
        }
    }
    snapped
}
