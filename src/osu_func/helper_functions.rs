// ---------- Helper Methods ----------
// cumulativeSum 计算累积积分 (对应Python的cumulative_sum)
fn cumulative_sum(x: &[f64], f: &[f64]) -> Vec<f64> {
    let mut res = vec![0.0; x.len()];
    for i in 1..x.len() {
        let dx = x[i] - x[i-1];
        res[i] = res[i-1] + f[i-1] * dx;
    }
    res
}

// 累积积分查询
fn query_cum_sum(q: f64, x: &[f64], res: &[f64], f: &[f64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    
    match (
        q <= x[0],
        q >= *x.last().unwrap()
    ) {
        (true, _) => 0.0,
        (_, true) => *res.last().unwrap(),
        _ => {
            let i = x.partition_point(|&v| v < q) - 1;
            res[i] + f[i] * (q - x[i])
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) enum SmoothMode {
    Sum,
    Average,
}

// smoothOnCorners 滑动窗口平滑 (对应Python的smooth_on_corners)
pub(super) fn smooth_on_corners(x: &[f64], f: &[f64], window: f64, scale: f64, mode: SmoothMode) -> Vec<f64> {
    let res = cumulative_sum(x, f);
    let mut g = vec![0.0; x.len()];

    for (i, &s) in x.iter().enumerate() {
        // 计算窗口边界
        let a = (s - window).max(x[0]);
        let b = (s + window).min(x[x.len() - 1]);

        // 计算积分值
        let val = query_cum_sum(b, x, &res, f) - query_cum_sum(a, x, &res, f);

        // 处理不同模式
        g[i] = match mode {
            SmoothMode::Average => {
                if (b - a).abs() > 1e-9 {
                    val / (b - a)
                } else {
                    0.0
                }
            },
            SmoothMode::Sum => scale * val,
        };
    }
    g
}

// interpValues 线性插值 (对应Python的interp_values)
pub(super) fn interp_values(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
    new_x.iter().map(|&x| {
        let idx = old_x.partition_point(|&v| v < x);
        match idx {
            0 => old_vals[0],
            n if n >= old_x.len() => *old_vals.last().unwrap(),
            _ => {
                let x0 = old_x[idx-1];
                let x1 = old_x[idx];
                let y0 = old_vals[idx-1];
                let y1 = old_vals[idx];
                let t = (x - x0) / (x1 - x0);
                y0 + t * (y1 - y0)
            }
        }
    }).collect()
}

// stepInterp 阶梯插值 (对应Python的step_interp)
pub(super) fn step_interp(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
    new_x.iter().map(|&x| {
        let idx  = old_x.partition_point(|&v| v <= x);
        let idx = idx.saturating_sub(1).min(old_vals.len() - 1);
        old_vals[idx]
    }).collect()
}

pub(super) fn rescale_high(sr: f64) -> f64 {
    if sr <= 9.0 {
        sr
    } else {
        9.0 + (sr - 9.0) * (1.0 / 1.2)
    }
}

pub(super) fn find_next_note_in_column(note: (u32, u32, i32), times: &[u32], note_seq_by_column: &[Vec<(u32, u32, i32)>]) -> (u32, u32, i32) {
    let (k, h, _) = note;
    let column_notes = note_seq_by_column.get(k as usize).unwrap();
    // 在当前列的时间序列中二分查找
    let idx = times.binary_search(&h).unwrap_or_else(|i| i);
    if idx + 1 < column_notes.len() {
        column_notes[idx + 1].clone()
    } else {
        // 返回默认值
        (0, 1e9 as u32, 1e9 as i32)
    }
}