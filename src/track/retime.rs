//! 配速曲线重定时。
//!
//! 生成器先按整体配速铺出一条几何轨迹，这里再按用户绘制的配速曲线重新
//! 分配每个点的时间戳：只改变配速的「形状」（哪里快、哪里慢），整体时长
//! 仍与用户设定的距离/平均配速一致。

use route_planner::profile::windows_from_cum;

use super::altitude::{sample_curve, validate_curve};
use super::geom::{fmt_gain_time, round_to};
use super::model::{Segment, TenWindow, Track};

/// 按进度曲线调整配速：x 为已完成距离占比（0~1），y 为目标配速（秒/km）。
pub fn apply_pace_curve(track: &mut Track, curve: &[(f64, f64)]) -> Result<(), String> {
    validate_curve(curve, 60.0, 3600.0, "配速")?;
    let n = track.locations.len();
    if n < 2 {
        return Err("轨迹点不足，无法应用配速曲线".into());
    }
    let total = track
        .totalDistance
        .max(track.locations[n - 1].totalDis);
    let total = if total > 1e-6 { total } else { 1.0 };
    let dur = track.totalTime.max(1) as f64;

    // 每段目标耗时：段距离 × 曲线在该段中点的配速。
    let mut raw = Vec::with_capacity(n - 1);
    for i in 1..n {
        let d0 = track.locations[i - 1].totalDis;
        let d1 = track.locations[i].totalDis;
        let dd = (d1 - d0).max(0.0);
        let x = (((d0 + d1) * 0.5) / total).clamp(0.0, 1.0);
        raw.push((dd / 1000.0) * sample_curve(curve, x));
    }
    let sum: f64 = raw.iter().sum();
    if !sum.is_finite() || sum <= 1e-9 {
        return Err("配速曲线数值无效".into());
    }

    // 整体缩放到原总时长，只保留曲线的相对形状。
    let factor = dur / sum;
    let mut times = Vec::with_capacity(n);
    times.push(0.0);
    let mut acc = 0.0;
    for dt in &raw {
        acc += dt * factor;
        times.push(acc);
    }
    for i in 1..n {
        if times[i] <= times[i - 1] {
            times[i] = times[i - 1] + 0.05;
        }
    }
    let last = *times.last().unwrap_or(&dur);
    if last > 0.0 && (last - dur).abs() > 0.5 {
        let scale = dur / last;
        for t in times.iter_mut() {
            *t *= scale;
        }
    }

    // 取整并保证严格递增（相邻点至少 1 秒）。
    let mut ints = Vec::with_capacity(n);
    for t in &times {
        ints.push(round_to(*t, 0) as i64);
    }
    for i in 1..n {
        if ints[i] <= ints[i - 1] {
            ints[i] = ints[i - 1] + 1;
        }
    }
    let total_time = ints[n - 1].max(1);
    let start_ms = track.startTime;
    let dists: Vec<f64> = track
        .locations
        .iter()
        .map(|point| point.totalDis)
        .collect();

    for (i, point) in track.locations.iter_mut().enumerate() {
        let seconds = ints[i] as f64;
        let old_avg = point.avgSpeed.max(1e-6);
        point.totalTime = ints[i];
        point.validTime = ints[i];
        point.gainTimeMs = start_ms + ints[i] * 1000;
        point.gainTime = fmt_gain_time(point.gainTimeMs);
        if seconds > 0.0 {
            point.avgSpeed = round_to(point.totalDis / seconds, 4);
        }
        // bdS 与平均速度同源，按比例跟随新的配速曲线。
        if old_avg > 0.0 {
            point.bdS = round_to(point.bdS * (point.avgSpeed / old_avg), 3);
        }
        // 瞬时速度（km/h）跟随曲线，App 显示的单点配速才会体现自定义形状。
        if i > 0 {
            let dd = (dists[i] - dists[i - 1]).max(0.0);
            let dt = (ints[i] - ints[i - 1]).max(1) as f64;
            point.speed = round_to(dd / dt * 3.6, 4);
        }
    }

    track.totalTime = total_time;
    track.validTime = total_time;

    let dur_f = total_time as f64;
    let cum_time: Vec<f64> = ints.iter().map(|v| *v as f64).collect();
    let cum_dist: Vec<f64> = track
        .locations
        .iter()
        .map(|point| point.totalDis)
        .collect();
    let cum_steps: Vec<f64> = track
        .locations
        .iter()
        .map(|point| point.steps as f64)
        .collect();

    track.speedPerTenSec = windows_from_cum(&cum_time, &cum_dist, dur_f, 10.0)
        .into_iter()
        .map(|(sec, d)| TenWindow {
            time: sec.round().max(1.0) as i64,
            value: round_to(d, 2),
        })
        .collect();
    track.stepsPerTenSec = windows_from_cum(&cum_time, &cum_steps, dur_f, 10.0)
        .into_iter()
        .map(|(sec, steps)| TenWindow {
            time: sec.round().max(1.0) as i64,
            value: round_to(steps, 0),
        })
        .collect();

    let mut segments: Vec<Segment> = Vec::new();
    let mut cursor = 0.0f64;
    for ((sec, d), (_, steps)) in windows_from_cum(&cum_time, &cum_dist, dur_f, 60.0)
        .into_iter()
        .zip(windows_from_cum(&cum_time, &cum_steps, dur_f, 60.0))
    {
        let start = cursor;
        cursor += sec;
        segments.push(Segment {
            totalTime: sec.round().max(1.0) as i64,
            distance: round_to(d, 0) as i64,
            startTime: round_to(start * 1000.0, 0) as i64,
            endTime: round_to(cursor * 1000.0, 0) as i64,
            avgSpeed: round_to(d / sec.max(1.0), 3),
            avgStep: round_to(steps / sec.max(1.0) * 60.0, 0) as i64,
            state: 0,
        });
    }
    track.segments = segments;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::generator::build;

    fn points() -> Vec<(f64, f64)> {
        vec![(38.901678, 121.540241), (38.902564, 121.541233)]
    }

    #[test]
    fn pace_curve_keeps_total_duration_and_monotonic_time() {
        let mut track = build(1200.0, 600, 7, (38.9, 121.54), 1_700_000_000_000, &points(), None)
            .expect("build");
        let curve = vec![(0.0, 420.0), (0.5, 330.0), (1.0, 400.0)];
        apply_pace_curve(&mut track, &curve).expect("apply");
        assert_eq!(track.totalTime, track.validTime);
        assert!(track.totalTime >= 590 && track.totalTime <= 640);
        for pair in track.locations.windows(2) {
            assert!(pair[1].totalTime > pair[0].totalTime);
            assert!(pair[1].gainTimeMs >= pair[0].gainTimeMs);
        }
        assert!(!track.segments.is_empty());
    }
}
