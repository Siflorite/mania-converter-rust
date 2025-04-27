use crate::osu_func::OsuData;
use crate::osu_func::helper_functions::*;

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io;
use std::borrow::Cow;

fn preprocess(osu_data: &OsuData, speed: f64) -> 
    Result<(
        f64, // x, a timing window param
        u32, // k, num of columns
        u32, // T, total time
        Vec<(u32, u32, i32)>, // note_seq, note sequence
        Vec<Vec<(u32, u32, i32)>>, // note_seq_by_column, note sequence grouped by column
        Vec<(u32, u32, u32)>, // ln_seq, long note sequence
        Vec<(u32, u32, u32)>, // tail_seq, long note sequence sorted by end time (tail time)
        Vec<Vec<(u32, u32, u32)>> // ln_seq_by_column, long note sequence grouped by column
    ), io::Error> {

    let time_multiplier = match speed {
        0.5..2.0 => 1.0 / speed,
        _ => 1.0,
    };

    if osu_data.misc.circle_size == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Circle size is 0, meaning no columns!")); 
    }

    let mut note_seq: Vec<(u32, u32, i32)> = osu_data.notes.iter()
        .map(|note| {
            let k = note.x_pos * osu_data.misc.circle_size / 512;
            let h = (note.time as f64 * time_multiplier) as u32;
            let t = note.end_time.map_or(-1, |x| (x as f64 * time_multiplier) as i32);
            (k, h, t)
        })
        .collect();

    let x = {
        let base = 0.3 * ((64.5 - (osu_data.misc.od * 3.0).ceil()) / 500.0).sqrt();
        base.min(0.6 * (base - 0.09) + 0.09)
    };

    note_seq.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    // Group notes by column
    let column_count = osu_data.misc.circle_size as usize;
    let mut note_seq_by_column = vec![Vec::new(); column_count];
    for &note in &note_seq {
        note_seq_by_column[note.0 as usize].push(note);
    }

    // Process long notes
    let ln_seq: Vec<(u32, u32, u32)> = note_seq.iter()
        .filter(|&&(_, _, t)| t >= 0)
        .map(|&(k, h, t)| (k, h, t as u32))
        .collect();
    
    let mut tail_seq = ln_seq.clone();
    tail_seq.sort_by(|a,b| {
        a.2.cmp(&b.2).then(a.1.cmp(&b.1)).then(a.0.cmp(&b.0))
    });

    let mut ln_seq_by_column = vec![Vec::new(); column_count];
    for &ln in &tail_seq {
        ln_seq_by_column[ln.0 as usize].push(ln);
    }

    let k = osu_data.misc.circle_size;
    let t = note_seq.iter()
        .map(|&(_, h, t)| h.max(if t >= 0 { t as u32 } else { 0 }))
        .max()
        .unwrap_or(0) + 1;

    Ok((x, k, t, note_seq, note_seq_by_column, ln_seq, tail_seq, ln_seq_by_column))
}

fn get_corners(total: u32, note_seq: &[(u32, u32, i32)]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let add_time = |set: &mut HashSet<u32>, time: u32, offset: i32| {
        let time_i64 = time as i64;
        let new_time = time_i64 + offset as i64;
        
        if new_time >= 0 && new_time <= total as i64 {
            set.insert(new_time as u32);
        }
    };

    let add_time_base = |set: &mut HashSet<u32>, time: u32| {
        add_time(set, time, 0);
        add_time(set, time, 1);
        add_time(set, time, 501);
        add_time(set, time, -499);
    };
    
    let mut corners_base_set = HashSet::new();
    for &(_, h, t) in note_seq {
        add_time_base(&mut corners_base_set,h);
        if t >= 0 {
            add_time_base(&mut corners_base_set, t as u32);
        }
    }
    corners_base_set.insert(0);
    corners_base_set.insert(total);
    let mut corners_base: Vec<u32> = corners_base_set.iter().cloned().collect();
    corners_base.sort_unstable();

    let add_time_a = |set: &mut HashSet<u32>, time: u32| {
        add_time(set, time, 0);
        add_time(set, time, 1000);
        add_time(set, time, -1000);
    };

    let mut corners_a_set = HashSet::new();
    for &(_, h, t) in note_seq {
        add_time_a(&mut corners_a_set,h);
        if t >= 0 {
            add_time_a(&mut corners_a_set, t as u32);
        }
    }
    corners_a_set.insert(0);
    corners_a_set.insert(total);
    let mut corners_a: Vec<u32> = corners_a_set.iter().cloned().collect();
    corners_a.sort_unstable();

    let corners_all_set: HashSet<u32> = corners_base_set.iter()
        .chain(corners_a_set.iter())
        .cloned()
        .collect();
    let mut corners_all = Vec::from_iter(corners_all_set);
    corners_all.sort_unstable();
    (corners_all, corners_base, corners_a)
}

fn get_key_usage(col: u32, total: u32, note_seq: &[(u32, u32, i32)], base_corners: &[u32]) -> Vec<Vec<bool>> {
    let mut key_usage = vec![vec![false; base_corners.len()]; col as usize];

    for &(k, h, t) in note_seq {
        let start_time = h.saturating_sub(150);
        let end_time = if t < 0 {
            h + 150
        } else {
            (t as u32 + 150).min(total - 1)
        };
        let left_idx = base_corners.partition_point(|&x| x < start_time);
        let right_idx = base_corners.partition_point(|&x| x < end_time);
        for idx in left_idx..right_idx {
            key_usage[k as usize][idx] = true;
        }
    }

    key_usage
}

fn get_key_usage_400(col: u32, total: u32, note_seq: &[(u32, u32, i32)], base_corners: &[u32]) -> Vec<Vec<f64>> {
    let mut key_usage_400 = vec![vec![0.0; base_corners.len()]; col as usize];
    const COEFF: f64  = 3.75 / (400.0 * 400.0);
    for &(k , h, t) in note_seq {
        let k = k as usize;
        let start_time = h.max(0);
        let end_time = if t < 0 {
           h 
        } else {
            (t as u32).min(total - 1)
        };
        let left_400_idx = base_corners.partition_point(|&x| x < start_time.saturating_sub(400));
        let left_idx = base_corners.partition_point(|&x| x < start_time);
        let right_idx = base_corners.partition_point(|&x| x < end_time);
        let right_400_idx = base_corners.partition_point(|&x| x < end_time + 400);
        let row = &mut key_usage_400[k];

        (left_idx..right_idx).for_each(|idx| {
            row[idx] += 3.75 + (end_time - start_time).min(1500) as f64 / 150.0;
        });
        (left_400_idx..left_idx).for_each(|idx| {
            let diff = (start_time - base_corners[idx]) as f64;
            row[idx] += 3.75 - COEFF * diff * diff;
        });
        (right_idx..right_400_idx).for_each(|idx| {
           let diff = (base_corners[idx] - end_time) as f64;
           row[idx] += 3.75 - COEFF * diff * diff; 
        });
    }

    key_usage_400
}

fn compute_anchor(_col: u32, key_usage_400: &[Vec<f64>], base_corners: &[u32]) -> Vec<f64> {
    let mut anchor = vec![0.0; base_corners.len()];
    
    for (idx, _) in base_corners.iter().enumerate() {
        // 收集当前索引的所有键的计数值
        // 因为Rust有iter和map，因此不用知道key_usage_400一维长度
        let mut counts: Vec<f64> = key_usage_400
            .iter()
            .map(|row| row[idx])
            .collect();
        
        // 降序排序
        counts.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap());
        
        // 过滤非零值
        let nonzero_counts: Vec<&f64> = counts
            .iter()
            .filter(|&&v| v.abs() > f64::EPSILON) // 考虑浮点误差
            .collect();
        
        let anchor_val = if nonzero_counts.len() > 1 {
            // 计算walk和max_walk
            let (walk, max_walk) = nonzero_counts
                .windows(2)
                .fold((0.0, 0.0), |(acc_w, acc_m), pair| {
                    let current = *pair[0];
                    let next = *pair[1];
                    let ratio = next / current;
                    let term = current * (1.0 - 4.0 * (0.5 - ratio).powi(2));
                    (acc_w + term, acc_m + current)
                });
            
            // 防止除零
            if max_walk < f64::EPSILON {
                0.0
            } else {
                walk / max_walk
            }
        } else { 0.0 };
        // 应用最终计算
        anchor[idx] = 1.0 + (anchor_val - 0.18).min(5.0 * (anchor_val - 0.22).powi(3));
    }
    
    anchor
}

fn ln_bodies_count_sparse_representation
    (ln_seq: &[(u32, u32, u32)], total: u32) -> (Vec<u32>, Vec<f64>, Vec<f64>) {
    let mut diff = BTreeMap::new();

    // 处理每个事件
    for &(_k, h, t) in ln_seq {
        let t0 = (h + 60).min(t);
        let t1 = (h + 120).min(t);
        *diff.entry(t0).or_insert(0.0) += 1.3;
        *diff.entry(t1).or_insert(0.0) += -0.3; // -1.3 + 1
        *diff.entry(t).or_insert(0.0) -= 1.0;
    }
    
    // 生成有序断点
    let mut points: Vec<u32> = diff.keys().cloned().collect();
    points.insert(0, 0);
    points.push(total);
    points.sort_unstable();
    points.dedup();
    
    // 构建分段常数值和累积和
    let mut values = Vec::with_capacity(points.len() - 1);
    let mut cumsum = vec![0.0];
    let mut curr: f64 = 0.0;
    
    for window in points.windows(2) {
        let &[t_start, t_end] = window else { unreachable!() };
        if let Some(delta) = diff.get(&t_start) {
            curr += delta;
        }
        // 计算转换后的值
        let v = curr.min(2.5 + 0.5 * curr);
        values.push(v);
        // 计算累积和
        let seg_length = (t_end - t_start) as f64;
        cumsum.push(cumsum.last().unwrap() + seg_length * v);
    }
    
    (points, cumsum, values)
}

/// 计算时间区间 [a, b) 内的累积值
/// 参数说明：
/// - a: 起始时间 (包含)
/// - b: 结束时间 (不包含)
/// - ln_rep: 元组包含 (断点, 累积和, 分段值)
fn ln_sum(a: u32, b: u32, ln_rep: (&[u32], &[f64], &[f64])) -> f64 {
    let (points, cumsum, values) = ln_rep;
    if points.is_empty() || a > b {
        return 0.0
    }

    // 实现类似 bisect_right 的功能
    let find_index = |x| {
        points.partition_point(|&p| p <= x).saturating_sub(1)
    };
    let i = find_index(a);
    let j = find_index(b);
    let i = i.clamp(0, points.len() - 2);
    let j = j.clamp(0, points.len() - 2);

    let total = if i == j {
        (b - a) as f64 * values[i]
    } else {
        // 首段部分
        let first_seg = (points[i+1] - a) as f64 * values[i];
        // 中间完整分段
        let middle_seg = cumsum[j] - cumsum[i+1];
        // 尾段部分
        let last_seg = (b - points[j]) as f64 * values[j];
        first_seg + middle_seg + last_seg
    };
    total.max(0.0)
}

fn compute_j_bar(col: u32, _total: u32, x: f64, note_seq_by_column: &[Vec<(u32, u32, i32)>], base_corners: &[f64]) -> (Vec<Vec<f64>>, Vec<f64>) {
    let len = base_corners.len();
    let mut j_ks = vec![vec![0.0; len]; col as usize];
    let mut delta_ks = vec![vec![1e9; len]; col as usize];
    let jack_nerfer = |delta: f64| {
        1.0 - 7e-5 * (0.15 + (delta - 0.08).abs()).powi(-4)
    };

    for (k, notes_column) in note_seq_by_column.iter().enumerate() {
       for pair in notes_column.windows(2) {
            let start = pair[0].1;
            let end = pair[1].1;

            let left_idx = base_corners.partition_point(|&t| t < start as f64);
            let right_idx = base_corners.partition_point(|&t| t  < end as f64);
            if left_idx >= right_idx {
                continue;
            }

            let delta = 0.001 * (end - start) as f64;
            let val = 1.0 / delta / (delta + 0.11 * x.powf(0.25));
            let j_val = val * jack_nerfer(delta);
            for idx in left_idx..right_idx {
                j_ks[k][idx] = j_val;
                delta_ks[k][idx] = delta;
            }
       } 
    }

    let j_bar_ks: Vec<Vec<f64>> = j_ks.iter().map(|j| {
        smooth_on_corners(&base_corners, j, 500.0, 0.001, SmoothMode::Sum)
    }).collect();

    let mut j_bar = vec![0.0; len];
    // for (i, _) in base_corners.iter().enumerate() {
    //     let vals = j_bar_ks.iter().map(|j| j[i]).collect::<Vec<f64>>(); 
    //     let weights = delta_ks.iter().map(|d| 1.0 / d[i]).collect::<Vec<f64>>();
    //     let num: f64 = zip(&vals, &weights).map(|(&v,&w)| {
    //         v.max(0.0).powi(5) * w
    //     }).sum();
    //     let den: f64 = weights.iter().sum();
    //     j_bar[i] = if den < f64::EPSILON { 0.0 } else { (num / den).powf(0.2) };
    // }
    for i in 0..len {
        let (mut num, mut den) = (0.0, 0.0);
        for k in 0..col as usize {
            let val = j_bar_ks[k][i].max(0.0);
            let weight = 1.0 / delta_ks[k][i];
            num += val.powi(5) * weight;
            den += weight;
        }
        j_bar[i] = if den < 1e-9 {
            0.0
        } else {
            (num/den).powf(0.2)
        }
    }

    (delta_ks, j_bar)
}

fn merge_sorted(a: &[(u32,u32,i32)], b: &[(u32,u32,i32)]) -> Vec<(u32,u32,i32)> {
    let mut merged: Vec<(u32,u32,i32)> = Vec::with_capacity(a.len() + b.len());
    let mut i = 0;
    let mut j = 0;

    while i < a.len() && j < b.len() {
        if a[i].1 <= b[j].1 {
            merged.push(a[i]);
            i += 1;
        } else {
            merged.push(b[j]);
            j += 1;
        }
    }
    merged.extend_from_slice(&a[i..]);
    merged.extend_from_slice(&b[j..]);
    merged
}

fn compute_x_bar(col: u32, _total: u32, x: f64, note_seq_by_column: &[Vec<(u32, u32, i32)>], active_columns: &[Vec<u32>], base_corners: &[f64]) -> Vec<f64> {
   // 交叉系数矩阵
   let col_u = col as usize;
   let corner_n = base_corners.len();
   let cross_matrix: [&'static [f64]; 11] = [
        &[-1.0],
        &[0.075, 0.075],
        &[0.125, 0.05, 0.125],
        &[0.125, 0.125, 0.125, 0.125],
        &[0.175, 0.25, 0.05, 0.25, 0.175],
        &[0.175, 0.25, 0.175, 0.175, 0.25, 0.175],
        &[0.225, 0.35, 0.25, 0.05, 0.25, 0.35, 0.225],
        &[0.225, 0.35, 0.25, 0.225, 0.225, 0.25, 0.35, 0.225],
        &[0.275, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.275],
        &[0.275, 0.45, 0.35, 0.25, 0.275, 0.275, 0.25, 0.35, 0.45, 0.275],
        &[0.325, 0.55, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.55, 0.325]
    ];
    let cross_coeff = cross_matrix[col_u];

    let mut x_ks = vec![vec![0.0; corner_n]; col_u + 1];
    let mut fast_cross = vec![vec![0.0; corner_n]; col_u + 1];

    for k in 0..=col_u {
        let notes: Cow<'_, [(u32, u32, i32)]> = match k{
            0 => Cow::Borrowed(&note_seq_by_column[0]),
            n if n == col_u => Cow::Borrowed(&note_seq_by_column[col_u - 1]),
            n => Cow::Owned(merge_sorted(&note_seq_by_column[n-1], &note_seq_by_column[n as usize]))
        };

        for pair in notes.windows(2) {
            let start = pair[0].1;
            let end = pair[1].1;

            let left_idx = base_corners.partition_point(|&t| t < start as f64);
            let right_idx = base_corners.partition_point(|&t| t < end as f64);

            if left_idx >= right_idx {
                continue; 
            }

            let delta = 0.001 * (end - start) as f64;
            let base_val = 0.16 * x.max(delta).powi(-2);
            let should_adjust = 
                (k == 0) ||
                (k > 0 && !active_columns[left_idx].contains(&(k as u32 - 1)) && !active_columns[right_idx].contains(&(k as u32 - 1)))
                || (!active_columns[left_idx].contains(&(k as u32)) && !active_columns[right_idx].contains(&(k as u32)));
            let val = if should_adjust {
                base_val * (1.0 - cross_coeff.get(k).unwrap_or(&0.0))
            } else {
                base_val
            };

            // 填充区间
            for idx in left_idx..right_idx {
                x_ks[k][idx] = val;
                let fc_val = 0.4 * delta.max(0.06).max(0.75 * x).powi(-2) - 80.0;
                fast_cross[k][idx] = fc_val.max(0.0);
            }
        }
    }

    let mut x_base = vec![0.0; corner_n];
    for i in 0..corner_n {
        // 第一部分求和
        let part1: f64 = (0..=col_u).map(|k| x_ks[k][i] * cross_coeff.get(k).unwrap_or(&0.0)).sum();
        
        // 第二部分求和
        let part2: f64 = (0..col_u).map(|k| {
            let a = fast_cross[k][i] * cross_coeff.get(k).unwrap_or(&0.0);
            let b = fast_cross[k+1][i] * cross_coeff.get(k+1).unwrap_or(&0.0);
            (a * b).sqrt()
        }).sum();

        x_base[i] = part1 + part2;
    }
    smooth_on_corners(base_corners, &x_base, 500.0, 0.001, SmoothMode::Sum)

}

fn compute_p_bar(_col: u32, _total: u32, x: f64, note_seq: &[(u32, u32, i32)], ln_rep: (&[u32], &[f64], &[f64]), anchor: &[f64], base_corners: &[f64]) -> Vec<f64> {
    let stream_booster = |delta: f64| {
        let ratio = 7.5 / delta;
        if ratio > 160.0 && ratio < 360.0 {
            1.0 + 1.7e-7 * (ratio - 160.0) * (ratio - 360.0) * (ratio - 360.0)
        } else {
            1.0
        }
    };
    
    let corner_len = base_corners.len();
    let mut p_step = vec![0.0; corner_len];
    for pair in note_seq.windows(2) {
        let start = pair[0].1;
        let end = pair[1].1;
        let delta_time = end - start;
        if delta_time == 0 { // Overlapping
            let spike = 1000.0 * (0.02 * (4.0 / x - 24.0)).powf(0.25);
            let left_idx = base_corners.partition_point(|&t| t < start as f64);
            let right_idx = base_corners.partition_point(|&t| t <= start as f64);
            for idx in left_idx..right_idx {
                p_step[idx] += spike;
            }
            continue;
        }
        let left_idx = base_corners.partition_point(|&t| t < start as f64);
        let right_idx = base_corners.partition_point(|&t| t < end as f64);
        if left_idx >= right_idx {
            continue;
        }
        
        let ln_sum = ln_sum(start as u32, end as u32, ln_rep);
        let delta = 0.001 * delta_time as f64;
        let v = 1.0 + 0.006 * ln_sum;
        let b_val = stream_booster(delta);
        let inc_base = if delta < (2.0 * x) / 3.0 {
            let term = 1.0 - 24.0 / x * (delta - x / 2.0).powi(2);
            0.08 / x * term
        } else {
            let term = 1.0 - 24.0 / x * (x / 6.0).powi(2);
            0.08 / x * term
        };
        let inc = (1.0 / delta) * inc_base.powf(0.25) * b_val.max(v);
        
        let max_inc = inc.max(inc * 2.0 - 10.0);
        for idx in left_idx..right_idx {
            p_step[idx] += (inc * anchor[idx]).min(max_inc);
        }
    }

    smooth_on_corners(base_corners, &p_step, 500.0, 0.001, SmoothMode::Sum)
}

fn compute_a_bar(col: u32, _total: u32, _x: f64, _note_seq_by_column: &[Vec<(u32, u32, i32)>], active_columns: &[Vec<u32>], delta_ks: &[Vec<f64>], a_corners: &[f64], base_corners: &[f64]) -> Vec<f64> {
    let col_u = col as usize;
    let corner_len = base_corners.len();
    let mut d_ks = vec![vec![0.0; corner_len]; col_u - 1];

    for (i, cols) in active_columns.iter().enumerate() {
        for j in 0..cols.len().saturating_sub(1) {
            let k0 = cols[j];
            let k1 = cols[j+1];
            let delta0 = delta_ks[k0 as usize][i];
            let delta1 = delta_ks[k1 as usize][i];
            let diff = (delta0 - delta1).abs();
            let max_delta = delta0.max(delta1);
            let base_val = 0.4 * (max_delta - 0.11).max(0.0);
            d_ks[k0 as usize][i] = diff + base_val;
        }
    }

    let mut a_step = vec![1.0; a_corners.len()];
    for (i, &s) in a_corners.iter().enumerate() {
        let idx = base_corners.partition_point(|&t| t < s)
            .min(corner_len.saturating_sub(1));
        let cols = &active_columns[idx];
        for j in 0..cols.len().saturating_sub(1) {
            let k0 = cols[j];
            let k1 = cols[j+1];
            let d_val = d_ks[k0 as usize][idx];
            let delta0 = delta_ks[k0 as usize][idx];
            let delta1 = delta_ks[k1 as usize][idx];
            let max_delta = delta0.max(delta1);
            if d_val < 0.02 {
                a_step[i] *= (0.75 + 0.5 * max_delta).min(1.0);
            } else if d_val < 0.07 {
                a_step[i] *= (0.65 + 0.5 * max_delta + 5.0 * d_val).min(1.0);
            }
        }
    }

    smooth_on_corners(a_corners, &a_step, 250.0, 1.0, SmoothMode::Average)
}

fn compute_r_bar(_col: u32, _total: u32, x: f64, note_seq_by_column: &[Vec<(u32, u32, i32)>], tail_seq: &[(u32, u32, u32)], base_corners: &[f64]) -> Vec<f64> {
    let corner_len = base_corners.len();
    let mut _i_arr = vec![0.0; corner_len]; // Not used
    let mut r_step = vec![0.0; corner_len];

    let times_by_column: Vec<Vec<u32>> = note_seq_by_column
        .iter().map(|column| {
            column.iter().map(|note| note.1)
        }.collect()).collect();
    
    let mut i_list: Vec<f64> = Vec::with_capacity(tail_seq.len());
    for &(k, h_i, t_i) in tail_seq.iter() {
        let (_, h_j, _) = find_next_note_in_column((k, h_i, t_i as i32), &times_by_column[k as usize], note_seq_by_column);
        let i_h =  0.001 * (t_i as f64 - h_i as f64 - 80.0).abs() / x;
        let i_t =  0.001 * (h_j as f64 - t_i as f64 - 80.0).abs() / x;
        let denominator = 2.0 + (-5.0 * (i_h - 0.75)).exp() + (-5.0 * (i_t - 0.75)).exp();
        i_list.push(2.0 / denominator);
    }

    for (i, pair) in tail_seq.windows(2).enumerate() {
        let (_, _, t_start) = pair[0];
        let (_, _, t_end) = pair[1];
        let left_idx = base_corners.partition_point(|&t| t < t_start as f64);
        let right_idx = base_corners.partition_point(|&t| t < t_end as f64);
        if left_idx >= right_idx {
            continue; 
        }
        // IArr updated in original algorithm but not used, so not implemented here, supposed to be:
        // i_arr[left_idx..right_idx] = 1 + i_list[i]

        let delta_r = 0.001 * (t_end - t_start) as f64;
        let r_val = 0.08 / delta_r.sqrt() / x * (1.0 + 0.8 * (i_list[i] + i_list[i+1]));
        for idx in left_idx..right_idx {
            r_step[idx] = r_val;
        }
    }
    
    smooth_on_corners(base_corners, &r_step, 500.0, 0.001, SmoothMode::Sum)
}

fn compute_c_and_ks(_col: u32, _total: u32, note_seq: &[(u32, u32, i32)], key_usage: &[Vec<bool>], base_corners: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let corners_len = base_corners.len();
    
    let mut note_hit_times: Vec<u32> = note_seq.iter().map(|note| note.1).collect();
    note_hit_times.sort_unstable(); // necessary?
    let c_step: Vec<f64> = base_corners.iter().map(|&t| {
        let low = (t as u32).saturating_sub(500);
        let high = t as u32 + 500;
        let idx_left = note_hit_times.partition_point(|&x| x < low);
        let idx_right = note_hit_times.partition_point(|&x| x < high);
        idx_right.saturating_sub(idx_left) as f64
    }).collect();

    let ks_step = (0..corners_len).map(|i| {
        let count= key_usage.iter().filter(|&column|{
            column[i]
        }).count().max(1);
        count as f64
    }).collect();

    (c_step, ks_step)
}

pub fn calculate_from_file(file_path: &str, speed: f64) -> io::Result<f64> {
    let data = OsuData::from_file(file_path)?;
    calculate_from_data(&data, speed)
}

pub fn calculate_from_data(data: &OsuData, speed: f64) -> io::Result<f64> {
    // ln_seq_by_column is not used in the calculation
    let (x, k, t, note_seq, note_seq_by_column, ln_seq, tail_seq, _ln_seq_by_column) = 
        preprocess(data, speed)?;
    
    let (corners_all, corners_base, corners_a) = get_corners(t, &note_seq);
    let base_corners_f: Vec<f64> = corners_base.iter().map(|&x| x as f64).collect();
    let a_corners_f: Vec<f64> = corners_a.iter().map(|&x| x as f64).collect();
    let all_corners_f: Vec<f64> = corners_all.iter().map(|&x| x as f64).collect();
    
    let key_usage = get_key_usage(k, t, &note_seq, &corners_base);
    let active_columns: Vec<Vec<u32>> = (0..corners_base.len()).map(|i| {
        (0..k).filter(|&c| {
            key_usage[c as usize][i]
        }).collect()
    }).collect();
    let key_usage_400 = get_key_usage_400(k, t, &note_seq, &corners_base);
    let anchor = compute_anchor(k, &key_usage_400, &corners_base);

    let (delta_ks, j_bar) = compute_j_bar(k, t, x, &note_seq_by_column, &base_corners_f);
    let j_bar_interp = interp_values(&all_corners_f, &base_corners_f, &j_bar);

    let x_bar = compute_x_bar(k, t, x, &note_seq_by_column, &active_columns, &base_corners_f);
    let x_bar_interp = interp_values(&all_corners_f, &base_corners_f, &x_bar);

    let ln_rep = ln_bodies_count_sparse_representation(&tail_seq, t);

    let p_bar = compute_p_bar(k, t, x, &note_seq, (&ln_rep.0, &ln_rep.1, &ln_rep.2), &anchor, &base_corners_f);
    let p_bar_interp = interp_values(&all_corners_f, &base_corners_f, &p_bar);

    let a_bar = compute_a_bar(k, t, x, &note_seq_by_column, &active_columns, &delta_ks, &a_corners_f, &base_corners_f);
    let a_bar_interp = interp_values(&all_corners_f, &a_corners_f, &a_bar);

    let r_bar = compute_r_bar(k, t, x, &note_seq_by_column, &tail_seq, &base_corners_f);
    let r_bar_interp = interp_values(&all_corners_f, &base_corners_f, &r_bar);

    let (c_step, ks_step) = compute_c_and_ks(k, t, &note_seq, &key_usage, &base_corners_f);
    let c_arr = step_interp(&all_corners_f, &base_corners_f, &c_step);
    let ks_arr = step_interp(&all_corners_f, &base_corners_f, &ks_step);

    // 计算最终难度指标
    let s_all: Vec<f64> = (0..all_corners_f.len()).map(|i| {
        let a_term_1 = a_bar_interp[i].powf(3.0 / ks_arr[i]);
        let j_term = j_bar_interp[i].min(8.0 + 0.85 * j_bar_interp[i]);
        let a_term_2 = a_bar_interp[i].powf(2.0 / 3.0);
        let leftover = 0.8 * p_bar_interp[i] + r_bar_interp[i] * 35.0/ (c_arr[i] + 8.0);
        0.4 * (a_term_1 * j_term).powf(1.5) + 0.6 * (a_term_2 * leftover).powf(1.5)
    }).map(|v| v.powf(2.0/3.0)).collect();

    let t_all: Vec<f64> = (0..all_corners_f.len()).map(|i| {
        (a_bar_interp[i].powf(3.0 / ks_arr[i]) * x_bar_interp[i]) / (x_bar_interp[i] + s_all[i] + 1.0)
    }).collect();

    let d_all: Vec<f64> = s_all.iter().zip(t_all.iter()).map(|(s, t)| {
        2.7 * s.powf(0.5) * t.powf(1.5) + s * 0.27
    }).collect();

    let mut gaps = vec![0.0; all_corners_f.len()];
    if all_corners_f.len() > 1 {
        gaps[0] = (all_corners_f[1] - all_corners_f[0]) / 2.0;
        gaps[all_corners_f.len()-1] = (all_corners_f[all_corners_f.len()-1] - all_corners_f[all_corners_f.len()-2]) / 2.0;
        for i in 1..all_corners_f.len()-1 {
            gaps[i] = (all_corners_f[i+1] - all_corners_f[i-1]) / 2.0;
        }
    }

    let effective_weights: Vec<f64> = c_arr.iter().zip(gaps.iter()).map(|(c, g)| c * g).collect();

    // 按照 d 值排序
    let mut sorted_data: Vec<(f64, f64)> = d_all.iter().zip(effective_weights.iter())
        .map(|(&d, &w)| (d, w))
        .collect();

    sorted_data.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let total_weight: f64 = effective_weights.iter().sum();
    let mut cum_weights = Vec::with_capacity(sorted_data.len());
    let mut acc = 0.0;
    for (_, w) in &sorted_data {
        acc += w;
        cum_weights.push(acc / total_weight);
    }

    let targets = [0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815];
    let indices: Vec<usize> = targets.iter().map(|t| {
        cum_weights.partition_point(|x| x < t)
    }).collect();

    // 计算最终 SR
    let percentile_93: f64 = indices[0..4].iter()
        .map(|&i| sorted_data[i].0)
        .sum::<f64>() / 4.0;
    
    let percentile_83: f64 = indices[4..8].iter()
        .map(|&i| sorted_data[i].0)
        .sum::<f64>() / 4.0;

    let weighted_mean: f64 = (sorted_data.iter()
        .map(|(d, w)| d.powf(5.0) * w)
        .sum::<f64>() / total_weight).powf(0.2);

    let mut sr = (0.88 * percentile_93) * 0.25 + 
                (0.94 * percentile_83) * 0.2 + 
                weighted_mean * 0.55;

    // 调整最终结果
    let total_notes = note_seq.len() as f64 + 
        ln_seq.iter().map(|&(_, h, t)| (t - h).min(1000) as f64 / 400.0).sum::<f64>();
    
    sr *= total_notes / (total_notes + 60.0);
    sr = rescale_high(sr);
    sr *= 0.975;

    Ok(sr)
}
