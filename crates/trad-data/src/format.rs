use crate::indicators;
use crate::market::OhlcvRow;

/// 报告生成选项
pub struct ReportOptions {
    pub tail: u32,
    pub indicators: bool,
    pub stats: bool,
}

/// 构建精简报告（对齐 Python build_compact_report）
///
/// 默认模式：指标快照表 + 尾部 OHLCV（默认30行）
/// `stats` 模式：附加区间统计
/// （`--raw` 纯 CSV 输出由调用方直接走 `ohlcv_to_csv`，不经过本函数）
pub fn build_compact_report(
    symbol: &str,
    start: &str,
    end: &str,
    data: &[OhlcvRow],
    opts: &ReportOptions,
) -> String {
    if data.is_empty() {
        return format!(
            "错误: 未获取到 {} 在 {} 至 {} 的数据，请检查代码和日期范围。",
            symbol, start, end
        );
    }

    let mut sections: Vec<String> = Vec::new();
    sections.push(format!("# {} 行情（{} 至 {}）\n", symbol, start, end));

    // 区间统计
    if opts.stats {
        sections.push(indicators::compute_stats(data));
    }

    // 技术指标快照
    if opts.indicators {
        sections.push(indicators::compute_indicators(data));
    }

    // 尾部 OHLCV
    let tail_n = opts.tail as usize;
    let tail_data = if data.len() > tail_n {
        &data[data.len() - tail_n..]
    } else {
        data
    };

    sections.push(format!("## 最近 {} 行 OHLCV\n", tail_data.len()));
    sections.push("```csv".to_string());
    // ohlcv_to_csv 末尾带 \n，这里 trim 掉避免代码块内出现空行
    sections.push(ohlcv_to_csv(tail_data).trim_end().to_string());
    sections.push("```\n".to_string());

    sections.join("\n")
}

/// 将 OhlcvRow 切片转为 CSV 字符串
///
/// 格式：Date,Open,High,Low,Close,Volume
///
/// 数值用 Rust 的 `{}`（最短往返表示）：API 字符串解析出的 f64 打印出来
/// 与原文一致（1363.02 → "1363.02"），不会出现 Python repr 式的
/// "100.30000000000001"；volume 同样不四舍五入——加密币成交量可以是小数。
/// 输出以单个 `\n` 结尾（--raw 模式直接 print! 时末行有换行符）。
pub fn ohlcv_to_csv(data: &[OhlcvRow]) -> String {
    let mut lines = Vec::new();
    lines.push("Date,Open,High,Low,Close,Volume".to_string());
    for row in data {
        lines.push(format!(
            "{},{},{},{},{},{}",
            row.date, row.open, row.high, row.low, row.close, row.volume
        ));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::OhlcvRow;

    fn row(date: &str, close: f64) -> OhlcvRow {
        OhlcvRow {
            date: date.to_string(),
            open: close - 0.5,
            high: close + 1.0,
            low: close - 1.0,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn test_empty_data() {
        let opts = ReportOptions {
            tail: 30,
            indicators: true,
            stats: false,
        };
        let result = build_compact_report("AAPL", "2024-01-01", "2024-06-30", &[], &opts);
        assert!(result.contains("错误"));
    }

    #[test]
    fn test_ohlcv_to_csv() {
        let data = vec![row("2024-01-01", 100.0)];
        let csv = ohlcv_to_csv(&data);
        assert!(csv.starts_with("Date,Open,High,Low,Close,Volume"));
        assert!(csv.contains("2024-01-01"));
        // 末行应以换行结尾（L15：--raw 输出末尾是 0a 而非无换行符）
        assert!(csv.ends_with('\n'));
    }

    #[test]
    fn test_tail_truncation() {
        let data: Vec<OhlcvRow> = (0..50).map(|i| row(&format!("d{}", i), 100.0)).collect();
        let opts = ReportOptions {
            tail: 10,
            indicators: false,
            stats: false,
        };
        let result = build_compact_report("TEST", "d0", "d49", &data, &opts);
        assert!(result.contains("最近 10 行"));
    }
}
