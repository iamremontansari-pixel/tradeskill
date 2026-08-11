use crate::http::get_with_retry_headers;
use crate::market::OhlcvRow;
use crate::yahoo::{append_crumb, get_crumb, BROWSER_UA};
use chrono::NaiveDate;
use reqwest::Client;
use serde_json::Value;

/// 将 YYYY-MM-DD 转为 Unix 时间戳（UTC 零点）
fn date_to_unix(date_str: &str) -> Result<i64, String> {
    let d = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| format!("日期解析失败 '{}': {}", date_str, e))?;
    let dt = d
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| format!("无效时间: '{}'", date_str))?;
    Ok(dt.and_utc().timestamp())
}

/// 解析 Yahoo Finance chart API 响应中的 OHLCV 数据
fn parse_yahoo_response(symbol: &str, body: &str) -> Result<Vec<OhlcvRow>, String> {
    let root: Value = serde_json::from_str(body).map_err(|e| format!("JSON 解析失败: {}", e))?;

    // 检查是否有错误信息。
    // 注意：chart.error 可能是 JSON null（区域封锁/缺 crumb 时常见），
    // 此时不视为显式错误，落到下方 "返回数据为空" 分支给出更明确的提示。
    if let Some(err) = root.get("chart").and_then(|c| c.get("error")) {
        if !err.is_null() {
            let code = err.get("code").and_then(|c| c.as_str());
            let desc = err.get("description").and_then(|d| d.as_str());
            let detail = match (code, desc) {
                (Some(c), Some(d)) => format!("{}: {}", c, d),
                (Some(c), None) => c.to_string(),
                (None, Some(d)) => d.to_string(),
                (None, None) => err.to_string(),
            };
            return Err(format!("Yahoo Finance 错误({}): {}", symbol, detail));
        }
    }

    let result = root
        .get("chart")
        .and_then(|c| c.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| {
            format!(
                "Yahoo Finance 返回数据为空({}): 可能 symbol 无效，或该地区访问受限（可尝试 --source eastmoney）",
                symbol
            )
        })?;

    let timestamps = result
        .get("timestamp")
        .and_then(|t| t.as_array())
        .ok_or("缺少 timestamp 字段")?;

    let indicators = result.get("indicators").ok_or("缺少 indicators 字段")?;

    let quote = indicators
        .get("quote")
        .and_then(|q| q.as_array())
        .and_then(|arr| arr.first())
        .ok_or("缺少 quote 数据")?;

    let opens = quote
        .get("open")
        .and_then(|v| v.as_array())
        .ok_or("缺少 open")?;
    let highs = quote
        .get("high")
        .and_then(|v| v.as_array())
        .ok_or("缺少 high")?;
    let lows = quote
        .get("low")
        .and_then(|v| v.as_array())
        .ok_or("缺少 low")?;
    let closes = quote
        .get("close")
        .and_then(|v| v.as_array())
        .ok_or("缺少 close")?;
    let volumes = quote
        .get("volume")
        .and_then(|v| v.as_array())
        .ok_or("缺少 volume")?;

    let mut rows = Vec::with_capacity(timestamps.len());
    // 辅助函数：从 JSON 数组中取 f64 值，null 则跳过（提升到循环外，避免每行重建闭包）
    let get_f64 =
        |arr: &Vec<Value>, idx: usize| -> Option<f64> { arr.get(idx).and_then(|v| v.as_f64()) };
    for (i, ts_val) in timestamps.iter().enumerate() {
        // 提取时间戳并转为日期字符串。
        // ts 非 i64（null/异常）或时间戳非法时跳过该行，避免生成 1970-01-01 假数据。
        let Some(ts) = ts_val.as_i64() else { continue };
        let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) else {
            continue;
        };
        let date = dt.format("%Y-%m-%d").to_string();

        // 如果关键字段为 null（停牌日），跳过该行
        let (Some(o), Some(h), Some(l), Some(c)) = (
            get_f64(opens, i),
            get_f64(highs, i),
            get_f64(lows, i),
            get_f64(closes, i),
        ) else {
            continue;
        };

        let vol = get_f64(volumes, i).unwrap_or(0.0);

        rows.push(OhlcvRow {
            date,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: vol,
        });
    }

    Ok(rows)
}

/// 单次 Yahoo chart 请求（query2 端点 + 浏览器 UA，可选 crumb）
async fn fetch_yahoo_chart(
    client: &Client,
    symbol: &str,
    period1: i64,
    period2: i64,
    crumb: Option<&str>,
) -> Result<Vec<OhlcvRow>, String> {
    let base = format!(
        "https://query2.finance.yahoo.com/v8/finance/chart/{}?period1={}&period2={}&interval=1d",
        crate::http::url_encode(symbol),
        period1,
        period2
    );
    let url = match crumb {
        Some(c) => append_crumb(&base, c),
        None => base,
    };

    let resp = get_with_retry_headers(client, &url, &[("User-Agent", BROWSER_UA)], Some(2))
        .await
        .map_err(|e| format!("Yahoo Finance 请求失败({}): {}", symbol, e))?;
    let body = crate::http::text_limited(resp).await?;
    parse_yahoo_response(symbol, &body)
}

/// 确定性错误：重试与 crumb 握手都不会改变结果（Invalid symbol / Not Found），
/// 直接返回，避免为无效 symbol 白做 2 次握手请求并掩盖原始错误。
fn is_definitive_yahoo_error(e: &str) -> bool {
    let lower = e.to_ascii_lowercase();
    lower.contains("invalid symbol")
        || lower.contains("not found")
        || lower.contains("no data found")
}

/// 按市场定制 Yahoo 失败提示：加密货币没有东方财富备用渠道，
/// 提示"改用 --source eastmoney"只会再撞一次"暂不支持加密货币"。
fn yahoo_fallback_hint(symbol: &str) -> String {
    match crate::market::detect_market(symbol) {
        crate::market::Market::Crypto => {
            "（加密货币无东方财富备用渠道，请检查网络后重试）".to_string()
        }
        _ => "（若所在地区无法访问 Yahoo，可改用 --source eastmoney）".to_string(),
    }
}

/// 获取美股/加密货币 OHLCV 数据（Yahoo Finance v8 API）
///
/// 调用 yfinance 底层协议：
/// `GET https://query2.finance.yahoo.com/v8/finance/chart/{symbol}`
///
/// 两步策略：
/// 1. 直连 query2（浏览器 UA）——多数地区一次成功；
/// 2. 仅当返回空数据或可恢复错误（传输/限流/区域封锁——数据中心 IP 下
///    Yahoo 常返回 HTTP 200 但 `chart.error=null`）才走 cookie + crumb 握手重试；
///    "Invalid symbol" 等确定性错误直接短路。
///
/// 日期参数从 YYYY-MM-DD 转为 Unix 时间戳。
/// 返回错误字符串而非 panic（对齐 Python "never raises" 契约）。
pub async fn fetch_us_ohlcv(
    client: &Client,
    symbol: &str,
    start: &str,
    end: &str,
) -> Result<Vec<OhlcvRow>, String> {
    let period1 = date_to_unix(start)?;
    // Yahoo 的 period2 是排他上界（timestamp < period2），而日线时间戳为当天 00:00 UTC。
    // 若直接用 end 当天 00:00 作 period2，会漏掉 end 当天的 K 线（默认 --end today 则丢当天）。
    // 加 1 天让端点包含在内，与东方财富通道（含端点）行为一致。
    let period2 = date_to_unix(end)? + 86_400;

    // 第一步：直连（无 crumb）。
    match fetch_yahoo_chart(client, symbol, period1, period2, None).await {
        Ok(rows) if !rows.is_empty() => return Ok(rows),
        // 确定性错误（Invalid symbol / Not Found 等）直接返回，不做徒劳的 crumb 握手
        Err(e) if is_definitive_yahoo_error(&e) => {
            return Err(format!("{}{}", e, yahoo_fallback_hint(symbol)));
        }
        // 空数据 / 传输限流 / 区域封锁 → 走 crumb 握手重试
        Ok(_) | Err(_) => {}
    }

    // 第二步：cookie + crumb 握手后重试。
    let crumb = get_crumb(client).await;
    let result = fetch_yahoo_chart(client, symbol, period1, period2, crumb.as_deref()).await;
    if result.is_err() {
        // 带 crumb 仍失败：crumb 可能已轮换/失效，清除缓存让后续请求重新握手
        crate::yahoo::invalidate_crumb_cache().await;
    }
    result.map_err(|e| format!("{}{}", e, yahoo_fallback_hint(symbol)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yahoo_happy_path() {
        let body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [1704153600, 1704240000],
                    "indicators": {
                        "quote": [{
                            "open": [100.0, 101.0],
                            "high": [102.0, 103.0],
                            "low": [99.0, 100.0],
                            "close": [101.0, 102.0],
                            "volume": [5000, 6000]
                        }]
                    }
                }],
                "error": null
            }
        }"#;
        let rows = parse_yahoo_response("AAPL", body).unwrap();
        assert_eq!(rows.len(), 2);
        assert!((rows[0].close - 101.0).abs() < 1e-9);
        assert!((rows[1].volume - 6000.0).abs() < 1e-9);
    }

    #[test]
    fn test_parse_yahoo_error_with_code_and_desc() {
        let body = r#"{"chart":{"result":null,"error":{"code":"Bad Request","description":"Invalid symbol"}}}"#;
        let err = parse_yahoo_response("XXX", body).unwrap_err();
        assert!(err.contains("Bad Request"));
        assert!(err.contains("Invalid symbol"));
        assert!(!err.contains("未知错误"));
    }

    #[test]
    fn test_parse_yahoo_error_code_only() {
        // 只有 code 没有 description —— 旧实现会输出 "未知错误"，现在应显示 code。
        let body = r#"{"chart":{"result":null,"error":{"code":"Too Many Requests"}}}"#;
        let err = parse_yahoo_response("AAPL", body).unwrap_err();
        assert!(err.contains("Too Many Requests"));
        assert!(!err.contains("未知错误"));
    }

    #[test]
    fn test_parse_yahoo_null_error_empty_result() {
        // chart.error 为 null 且 result 为 null：区域封锁/缺 crumb 的典型响应。
        // 不应再报 "未知错误"，而是 "返回数据为空" 提示。
        let body = r#"{"chart":{"result":null,"error":null}}"#;
        let err = parse_yahoo_response("AAPL", body).unwrap_err();
        assert!(err.contains("返回数据为空"));
        assert!(!err.contains("未知错误"));
    }

    #[test]
    fn test_parse_yahoo_skips_null_ohlc() {
        // 停牌日 OHLC 为 null 应跳过该行
        let body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [1704153600, 1704240000],
                    "indicators": {
                        "quote": [{
                            "open": [100.0, null],
                            "high": [102.0, null],
                            "low": [99.0, null],
                            "close": [101.0, null],
                            "volume": [5000, null]
                        }]
                    }
                }],
                "error": null
            }
        }"#;
        let rows = parse_yahoo_response("AAPL", body).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_parse_yahoo_skips_null_timestamp() {
        // timestamp 为 null 的行应跳过，而非生成 1970-01-01 假数据
        let body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [null, 1704240000],
                    "indicators": {
                        "quote": [{
                            "open": [100.0, 101.0],
                            "high": [102.0, 103.0],
                            "low": [99.0, 100.0],
                            "close": [101.0, 102.0],
                            "volume": [5000, 6000]
                        }]
                    }
                }],
                "error": null
            }
        }"#;
        let rows = parse_yahoo_response("AAPL", body).unwrap();
        assert_eq!(rows.len(), 1, "null timestamp 行应被跳过");
        assert_eq!(rows[0].date, "2024-01-03");
    }

    // 真实网络集成测试：默认不进 CI，手动 `cargo test -- --ignored` 运行。
    #[tokio::test]
    #[ignore = "hits the live Yahoo Finance API"]
    async fn test_live_yahoo_aapl() {
        let client = crate::http::build_client().unwrap();
        let rows = fetch_us_ohlcv(&client, "AAPL", "2024-01-01", "2024-06-30")
            .await
            .expect("Yahoo AAPL 应返回数据");
        assert!(!rows.is_empty(), "Yahoo AAPL 数据不应为空");
    }
}
