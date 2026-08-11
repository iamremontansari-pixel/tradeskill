use crate::market::OhlcvRow;

// 技术指标计算模块
//
// 严格对齐 Python `compute_indicators()` (fetch_stock_data.py 第62-223行)
// 所有边缘情况处理与 Python 一致。

/// 格式化数值：round 到 4 位小数，非有限浮点数返回 "N/A"。
///
/// 显式 `(v * 10000).round()` 强制「四舍五入（远离零）」；单独 `format!("{:.4}", v)`
/// 走的是银行家舍入（round-half-to-even），在末位恰好为 5 时结果会不同。保留显式
/// round 以维持当前输出，切勿简化为直接 `{:.4}`。
fn fmt_val(v: f64) -> String {
    // 先乘后判：v 接近 f64::MAX 时 v*10000 会溢出为 inf，is_finite 检查必须在乘法之后
    let scaled = v * 10000.0;
    if scaled.is_finite() {
        format!("{:.4}", scaled.round() / 10000.0)
    } else {
        "N/A".to_string()
    }
}

/// 计算 SMA（简单移动平均）
fn sma(data: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; data.len()];
    if data.len() < period {
        return result;
    }
    let mut sum = 0.0;
    for &val in data.iter().take(period) {
        sum += val;
    }
    result[period - 1] = sum / period as f64;
    for i in period..data.len() {
        sum += data[i] - data[i - period];
        result[i] = sum / period as f64;
    }
    result
}

/// 计算 EMA（指数移动平均，adjust=False，即递推式）
///
/// 与 Python `ewm(span=N, adjust=False)` 一致：
/// alpha = 2 / (span + 1)
/// ema[0] = data[0]
/// ema[i] = alpha * data[i] + (1 - alpha) * ema[i-1]
fn ema(data: &[f64], span: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; data.len()];
    if data.is_empty() {
        return result;
    }
    let alpha = 2.0 / (span as f64 + 1.0);
    result[0] = data[0];
    for i in 1..data.len() {
        result[i] = alpha * data[i] + (1.0 - alpha) * result[i - 1];
    }
    result
}

/// 计算 EMA with alpha parameter（用于 RSI/ATR）
///
/// 与 Python `ewm(alpha=A, adjust=False)` 一致：
/// ema[0] = data[0]
/// ema[i] = alpha * data[i] + (1 - alpha) * ema[i-1]
fn ema_alpha(data: &[f64], alpha: f64) -> Vec<f64> {
    let mut result = vec![f64::NAN; data.len()];
    if data.is_empty() {
        return result;
    }
    result[0] = data[0];
    for i in 1..data.len() {
        result[i] = alpha * data[i] + (1.0 - alpha) * result[i - 1];
    }
    result
}

/// 带删除的 Welford 在线统计器（窗口均值 + 平方偏差和 M2）。
///
/// 相对 sum/sum_sq 增量法的优势：不直接累计 `x²`，避免大数级序列上
/// `sum_sq - mean²·n` 的 catastrophic cancellation。
#[derive(Default)]
struct Welford {
    n: usize,
    mean: f64,
    m2: f64,
}

impl Welford {
    /// 加入一个新值（在线 Welford 更新）。
    fn add(&mut self, x: f64) {
        self.n += 1;
        let delta = x - self.mean;
        self.mean += delta / self.n as f64;
        self.m2 += delta * (x - self.mean);
    }

    /// 移除一个旧值（逆 Welford 更新；保证 n 先于 mean/m2 递减）。
    fn remove(&mut self, x: f64) {
        self.n -= 1;
        let delta = x - self.mean;
        self.mean -= delta / self.n as f64;
        self.m2 -= delta * (x - self.mean);
    }

    /// 总体标准差（ddof=0）；浮点噪声下 M2 可能为微小负值，用 max(0.0) 兜底。
    fn population_std(&self) -> f64 {
        (self.m2 / self.n as f64).max(0.0).sqrt()
    }
}

/// 计算 rolling std（总体标准差，ddof=0）
///
/// 用带删除的 Welford 在线算法维护窗口均值与平方偏差和，把 O(n·period)
/// 降到 O(n)，同时避免 sum/sum_sq 增量法在大数级序列上的
/// catastrophic cancellation（输出 0/负数）。仍保留 max(0.0) 兜底。
fn rolling_std(data: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; data.len()];
    if data.len() < period {
        return result;
    }
    let mut w = Welford::default();
    for &val in data.iter().take(period) {
        w.add(val);
    }
    result[period - 1] = w.population_std();
    for i in period..data.len() {
        w.add(data[i]);
        w.remove(data[i - period]);
        result[i] = w.population_std();
    }
    result
}

/// 计算 rolling mean
fn rolling_mean(data: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; data.len()];
    if data.len() < period {
        return result;
    }
    let mut sum = 0.0;
    for &val in data.iter().take(period) {
        sum += val;
    }
    result[period - 1] = sum / period as f64;
    for i in period..data.len() {
        sum += data[i] - data[i - period];
        result[i] = sum / period as f64;
    }
    result
}

/// 计算 rolling sum
fn rolling_sum(data: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; data.len()];
    if data.len() < period {
        return result;
    }
    let mut sum = 0.0;
    for &val in data.iter().take(period) {
        sum += val;
    }
    result[period - 1] = sum;
    for i in period..data.len() {
        sum += data[i] - data[i - period];
        result[i] = sum;
    }
    result
}

/// 计算 True Range
fn true_range(high: &[f64], low: &[f64], close: &[f64]) -> Vec<f64> {
    let n = high.len();
    let mut tr = vec![0.0; n];
    if n == 0 {
        return tr;
    }
    tr[0] = high[0] - low[0];
    for i in 1..n {
        let hl = high[i] - low[i];
        let hc = (high[i] - close[i - 1]).abs();
        let lc = (low[i] - close[i - 1]).abs();
        tr[i] = hl.max(hc).max(lc);
    }
    tr
}

/// 计算技术指标并生成 Markdown 快照表
///
/// 严格对齐 Python `compute_indicators()` 输出格式。
/// 指标：SMA(50/200)、EMA(10)、MACD(12,26,9)、RSI(14)、
/// Bollinger(20,2)、ATR(14)、VWMA(20)、MFI(14)。
pub fn compute_indicators(data: &[OhlcvRow]) -> String {
    if data.len() < 4 {
        return "## 技术指标\n\n> 数据列不全，无法计算指标。\n".to_string();
    }

    let close: Vec<f64> = data.iter().map(|r| r.close).collect();
    let high: Vec<f64> = data.iter().map(|r| r.high).collect();
    let low: Vec<f64> = data.iter().map(|r| r.low).collect();
    let volume: Vec<f64> = data.iter().map(|r| r.volume).collect();
    let n = close.len();
    let last = n - 1;

    // 移动平均
    let sma50 = sma(&close, 50);
    let sma200 = sma(&close, 200);
    let ema10 = ema(&close, 10);

    // MACD：12/26 EMA 差值，信号线为 9 EMA
    let ema12 = ema(&close, 12);
    let ema26 = ema(&close, 26);
    let macd: Vec<f64> = (0..n).map(|i| ema12[i] - ema26[i]).collect();
    let signal = ema(&macd, 9);
    let hist: Vec<f64> = (0..n).map(|i| macd[i] - signal[i]).collect();

    // RSI(14)：用 ewm(alpha=1/14, adjust=False) 近似
    let delta: Vec<f64> = {
        let mut d = vec![0.0; n];
        for i in 1..n {
            d[i] = close[i] - close[i - 1];
        }
        d
    };
    let gain: Vec<f64> = delta
        .iter()
        .map(|&d| if d > 0.0 { d } else { 0.0 })
        .collect();
    let loss: Vec<f64> = delta
        .iter()
        .map(|&d| if d < 0.0 { -d } else { 0.0 })
        .collect();
    let avg_gain = ema_alpha(&gain, 1.0 / 14.0);
    let avg_loss = ema_alpha(&loss, 1.0 / 14.0);

    // RSI 计算：处理 avg_loss == 0 的边缘情况
    // avg_gain==0 && avg_loss==0（价格连续持平）应为中性 50，而非 100；
    // 仅 avg_loss==0 且有上涨时才是 100（超买）。与下方 MFI 的处理一致。
    let rsi: Vec<f64> = (0..n)
        .map(|i| {
            let g = avg_gain[i];
            let l = avg_loss[i];
            if g == 0.0 && l == 0.0 {
                50.0
            } else if l == 0.0 {
                100.0
            } else {
                100.0 - 100.0 / (1.0 + g / l)
            }
        })
        .collect();

    // Bollinger(20, 2)：ddof=0（总体标准差）
    let boll_mid = rolling_mean(&close, 20);
    let boll_std = rolling_std(&close, 20);
    let boll_ub: Vec<f64> = (0..n).map(|i| boll_mid[i] + 2.0 * boll_std[i]).collect();
    let boll_lb: Vec<f64> = (0..n).map(|i| boll_mid[i] - 2.0 * boll_std[i]).collect();

    // ATR(14)：Wilder 平滑的 True Range
    let tr = true_range(&high, &low, &close);
    let atr = ema_alpha(&tr, 1.0 / 14.0);

    // VWMA(20)：成交量加权移动平均
    let cv: Vec<f64> = (0..n).map(|i| close[i] * volume[i]).collect();
    let cv_sum = rolling_sum(&cv, 20);
    let vol_sum = rolling_sum(&volume, 20);
    let vwma: Vec<f64> = (0..n)
        .map(|i| {
            if vol_sum[i].is_nan() || vol_sum[i] == 0.0 {
                f64::NAN
            } else {
                cv_sum[i] / vol_sum[i]
            }
        })
        .collect();

    // MFI(14)：资金流量指标
    let tp: Vec<f64> = (0..n)
        .map(|i| (high[i] + low[i] + close[i]) / 3.0)
        .collect();
    let mf: Vec<f64> = (0..n).map(|i| tp[i] * volume[i]).collect();
    let pos: Vec<f64> = (0..n)
        .map(|i| {
            if i == 0 {
                0.0
            } else if tp[i] > tp[i - 1] {
                mf[i]
            } else {
                0.0
            }
        })
        .collect();
    let neg: Vec<f64> = (0..n)
        .map(|i| {
            if i == 0 {
                0.0
            } else if tp[i] < tp[i - 1] {
                mf[i]
            } else {
                0.0
            }
        })
        .collect();
    let pos_sum = rolling_sum(&pos, 14);
    let neg_sum = rolling_sum(&neg, 14);
    let mfi: Vec<f64> = (0..n)
        .map(|i| {
            let ps = pos_sum[i];
            let ns = neg_sum[i];
            if ps.is_nan() || ns.is_nan() {
                return f64::NAN;
            }
            if ps == 0.0 && ns == 0.0 {
                50.0
            } else if ns == 0.0 && ps > 0.0 {
                100.0
            } else if ps == 0.0 && ns > 0.0 {
                0.0
            } else {
                100.0 - 100.0 / (1.0 + ps / ns)
            }
        })
        .collect();

    // 取最后一行的最新值
    let px = close[last];

    // 趋势信号判定
    // 金叉/死叉：SMA50 vs SMA200
    let cross = if sma50[last].is_nan() || sma200[last].is_nan() {
        "N/A".to_string()
    } else if sma50[last] > sma200[last] {
        "金叉(多头排列)".to_string()
    } else {
        "死叉(空头排列)".to_string()
    };

    // RSI 超买/超卖
    let rsi_state = if rsi[last].is_nan() {
        "中性".to_string()
    } else if rsi[last] >= 70.0 {
        "超买".to_string()
    } else if rsi[last] <= 30.0 {
        "超卖".to_string()
    } else {
        "中性".to_string()
    };

    // 布林位置
    let boll_pos = if boll_ub[last].is_nan() || boll_lb[last].is_nan() {
        "中轨附近".to_string()
    } else if px >= boll_ub[last] {
        "触及/突破上轨(超买区)".to_string()
    } else if px <= boll_lb[last] {
        "触及/跌破下轨(超卖区)".to_string()
    } else {
        "中轨附近".to_string()
    };

    // MACD 方向
    let macd_state = if macd[last].is_nan() || signal[last].is_nan() {
        "N/A".to_string()
    } else if macd[last] > signal[last] {
        "MACD>信号(多头)".to_string()
    } else {
        "MACD<信号(空头)".to_string()
    };

    // 组装紧凑指标快照表
    let lines: Vec<String> = vec![
        "## 技术指标快照（预计算）\n".to_string(),
        "| 指标 | 最新值 | 信号 |".to_string(),
        "|---|---|---|".to_string(),
        format!("| 收盘价 | {} | — |", fmt_val(px)),
        format!(
            "| SMA50 / SMA200 | {} / {} | {} |",
            fmt_val(sma50[last]),
            fmt_val(sma200[last]),
            cross
        ),
        format!("| EMA10 | {} | 短期动能 |", fmt_val(ema10[last])),
        format!(
            "| MACD / 信号 / 柱 | {} / {} / {} | {} |",
            fmt_val(macd[last]),
            fmt_val(signal[last]),
            fmt_val(hist[last]),
            macd_state
        ),
        format!("| RSI(14) | {} | {} |", fmt_val(rsi[last]), rsi_state),
        format!(
            "| Boll 中轨/上轨/下轨 | {} / {} / {} | {} |",
            fmt_val(boll_mid[last]),
            fmt_val(boll_ub[last]),
            fmt_val(boll_lb[last]),
            boll_pos
        ),
        format!("| ATR(14) | {} | 波动率参考 |", fmt_val(atr[last])),
        format!("| VWMA(20) | {} | 量价趋势 |", fmt_val(vwma[last])),
        format!("| MFI(14) | {} | 资金流向 |", fmt_val(mfi[last])),
    ];

    lines.join("\n") + "\n"
}

/// 计算区间统计（对齐 Python compute_stats）
pub fn compute_stats(data: &[OhlcvRow]) -> String {
    if data.is_empty() {
        return "## 区间统计\n\n> 数据不足。\n".to_string();
    }

    let close: Vec<f64> = data.iter().map(|r| r.close).collect();
    let volume: Vec<f64> = data.iter().map(|r| r.volume).collect();
    let n = close.len();

    let first = close[0];
    let last = close[n - 1];
    let ret = if first != 0.0 {
        (last / first - 1.0) * 100.0
    } else {
        f64::NAN
    };

    // 日百分比收益 → 年化波动率（252 交易日）
    let daily_ret: Vec<f64> = (1..n)
        .map(|i| {
            if close[i - 1] != 0.0 {
                (close[i] - close[i - 1]) / close[i - 1]
            } else {
                0.0
            }
        })
        .collect();

    let vol = if daily_ret.len() > 1 {
        let mean = daily_ret.iter().sum::<f64>() / daily_ret.len() as f64;
        let variance = daily_ret.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
            / (daily_ret.len() - 1) as f64;
        variance.sqrt() * (252.0_f64).sqrt() * 100.0
    } else {
        f64::NAN
    };

    let avg_vol = if !volume.is_empty() {
        volume.iter().sum::<f64>() / volume.len() as f64
    } else {
        f64::NAN
    };

    // 52 周高低：取最近约 252 个交易日
    let window = if n >= 252 {
        &close[n - 252..]
    } else {
        &close[..]
    };
    let hi = window.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let lo = window.iter().cloned().fold(f64::INFINITY, f64::min);

    fn fmt_num(v: f64, digits: u32) -> String {
        if v.is_finite() {
            let factor = 10.0_f64.powi(digits as i32);
            format!(
                "{:.prec$}",
                (v * factor).round() / factor,
                prec = digits as usize
            )
        } else {
            "N/A".to_string()
        }
    }

    let lines: Vec<String> = vec![
        "## 区间统计\n".to_string(),
        format!("- 区间收益率: {}%", fmt_num(ret, 2)),
        format!("- 年化波动率: {}%", fmt_num(vol, 2)),
        format!(
            "- 日均成交量: {}",
            if avg_vol.is_finite() {
                format!("{}", avg_vol as i64)
            } else {
                "N/A".to_string()
            }
        ),
        format!(
            "- 52周(或区间)高/低: {} / {}",
            fmt_num(hi, 4),
            fmt_num(lo, 4)
        ),
    ];

    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::OhlcvRow;

    fn row(date: &str, close: f64) -> OhlcvRow {
        OhlcvRow {
            date: date.to_string(),
            open: close,
            high: close + 1.0,
            low: close - 1.0,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn test_fmt_val() {
        assert_eq!(fmt_val(1.23456), "1.2346");
        assert_eq!(fmt_val(f64::NAN), "N/A");
        assert_eq!(fmt_val(f64::INFINITY), "N/A");
        assert_eq!(fmt_val(f64::NEG_INFINITY), "N/A");
        assert_eq!(fmt_val(0.0), "0.0000");
        // v*10000 溢出为 inf → N/A（修复前输出字面 "inf"）
        assert_eq!(fmt_val(f64::MAX), "N/A");
        assert_eq!(fmt_val(1e306), "N/A");
    }

    #[test]
    fn test_rolling_std_matches_bruteforce() {
        // 增量实现应与逐窗口直算一致（随机数据，含负值）
        let data: Vec<f64> = (0..50).map(|i| (i as f64 * 7.0).sin() * 100.0).collect();
        let inc = rolling_std(&data, 10);
        for i in 9..data.len() {
            let window = &data[i + 1 - 10..=i];
            let mean = window.iter().sum::<f64>() / 10.0;
            let variance = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 10.0;
            assert!(
                (inc[i] - variance.sqrt()).abs() < 1e-9,
                "窗口 {i} 不匹配: {} vs {}",
                inc[i],
                variance.sqrt()
            );
        }
    }

    #[test]
    fn test_sma_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sma(&data, 3);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 2.0).abs() < 1e-10);
        assert!((result[3] - 3.0).abs() < 1e-10);
        assert!((result[4] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_sma_insufficient_data() {
        let data = vec![1.0, 2.0];
        let result = sma(&data, 5);
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn test_ema_basic() {
        let data = vec![1.0, 2.0, 3.0];
        let result = ema(&data, 3);
        // alpha = 2/(3+1) = 0.5
        assert!((result[0] - 1.0).abs() < 1e-10);
        assert!((result[1] - 1.5).abs() < 1e-10);
        assert!((result[2] - 2.25).abs() < 1e-10);
    }

    #[test]
    fn test_ema_empty() {
        let result = ema(&[], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_rolling_std_large_values() {
        // 大数级序列（~1e6）叠加微小波动：sum/sum_sq 增量法因 catastrophic
        // cancellation 会把方差算成 0 或负数；Welford 应输出 > 0 且与直算一致。
        let base = 1_000_000.0;
        let data: Vec<f64> = (0..1000)
            .map(|i| base + ((i % 5) as f64 - 2.0) * 0.001)
            .collect();
        let result = rolling_std(&data, 20);
        for i in 19..data.len() {
            let window = &data[i + 1 - 20..=i];
            let mean = window.iter().sum::<f64>() / 20.0;
            let variance = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 20.0;
            let expected = variance.sqrt();
            assert!(
                result[i] > 0.0,
                "窗口 {i} 输出非正: {}（catastrophic cancellation）",
                result[i]
            );
            assert!(
                (result[i] - expected).abs() < 1e-6,
                "窗口 {i} 与直算不符: {} vs {}",
                result[i],
                expected
            );
        }
    }

    #[test]
    fn test_rolling_std_constant() {
        let data = vec![5.0, 5.0, 5.0, 5.0];
        let result = rolling_std(&data, 3);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 0.0).abs() < 1e-10);
        assert!((result[3] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_true_range() {
        let high = vec![10.0, 12.0];
        let low = vec![8.0, 9.0];
        let close = vec![9.0, 11.0];
        let tr = true_range(&high, &low, &close);
        assert!((tr[0] - 2.0).abs() < 1e-10); // 10-8
        assert!((tr[1] - 3.0).abs() < 1e-10); // max(12-9, |12-9|, |9-9|) = 3
    }

    #[test]
    fn test_compute_indicators_insufficient_data() {
        let data = vec![row("2024-01-01", 100.0), row("2024-01-02", 101.0)];
        let result = compute_indicators(&data);
        assert!(result.contains("数据列不全"));
    }

    #[test]
    fn test_compute_indicators_has_table() {
        let data: Vec<OhlcvRow> = (0..60)
            .map(|i| {
                row(
                    &format!("2024-{:02}-{:02}", i / 28 + 1, i % 28 + 1),
                    100.0 + i as f64,
                )
            })
            .collect();
        let result = compute_indicators(&data);
        assert!(result.contains("技术指标快照（预计算）"));
        assert!(result.contains("SMA50"));
        assert!(result.contains("RSI(14)"));
    }

    #[test]
    fn test_rsi_flat_market_is_neutral() {
        // 价格完全持平：avg_gain==avg_loss==0，RSI 应为 50（中性），而非 100。
        let data: Vec<OhlcvRow> = (0..30)
            .map(|i| row(&format!("2024-01-{:02}", i + 1), 100.0))
            .collect();
        let result = compute_indicators(&data);
        let rsi_line = result
            .lines()
            .find(|l| l.contains("RSI(14)"))
            .expect("应含 RSI 行");
        assert!(
            rsi_line.contains("| 50.0000 |"),
            "持平市场 RSI 应为 50.0000: {}",
            rsi_line
        );
    }

    #[test]
    fn test_compute_stats_empty() {
        let result = compute_stats(&[]);
        assert!(result.contains("数据不足"));
    }

    #[test]
    fn test_compute_stats_basic() {
        let data: Vec<OhlcvRow> = (0..10)
            .map(|i| row(&format!("2024-01-{:02}", i + 1), 100.0 + i as f64))
            .collect();
        let result = compute_stats(&data);
        assert!(result.contains("区间收益率"));
        assert!(result.contains("年化波动率"));
    }
}
