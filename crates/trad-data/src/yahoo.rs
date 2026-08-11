//! Yahoo Finance 共享反爬握手（cookie + crumb + 浏览器 UA）。
//!
//! Yahoo 的 chart (v8) 与 quoteSummary (v10) 等端点在数据中心 / 海外 IP 上
//! 需要 crumb token，否则返回 401 / 403，或 HTTP 200 但空数据。本模块把
//! 握手逻辑抽成共享代码，供 `market/us.rs`（行情）、`fundamentals.rs`、
//! `news.rs`（quoteSummary）复用。
use crate::http::get_with_retry_headers;
use reqwest::Client;
use std::time::Instant;
use tokio::sync::Mutex;

/// 浏览器 User-Agent：Yahoo 会封锁非浏览器 UA（尤其数据中心 IP），
/// 所有 Yahoo 请求必须携带真实浏览器 UA。
pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Crumb 缓存 TTL（秒）：Yahoo crumb 有效期数小时，1 小时后主动刷新避免使用过期 crumb。
const CRUMB_TTL_SECS: u64 = 3600;

/// 进程级 crumb 缓存：批量抓多个 symbol 时只握手一次（每次握手 2 个请求）。
/// 用 `tokio::sync::Mutex` 串行化整个「查缓存 → TTL 过期则刷新」流程，避免
/// 并发任务同时发现过期而各自握手（TOCTOU 双取）；tokio Mutex 允许在持有锁
/// 时 `.await` 握手。带 crumb 请求失败时调用方应 `invalidate_crumb_cache()`，
/// 让后续请求重新握手。
static CRUMB_CACHE: Mutex<Option<(String, Instant)>> = Mutex::const_new(None);

/// 清除 crumb 缓存（带 crumb 请求连续失败时调用，让下一个请求重新握手）
pub async fn invalidate_crumb_cache() {
    *CRUMB_CACHE.lock().await = None;
}

/// 获取 Yahoo Finance crumb token（yfinance 底层反爬握手）
///
/// 流程：先访问 fc.yahoo.com 种 cookie（cookie store 自动保存），
/// 再用同一 cookie 请求 v1/test/getcrumb 取得 crumb 字符串。
/// 任一步失败返回 None（调用方仍可尝试无 crumb 请求）。
/// 成功后写入进程级缓存（含 TTL）；`invalidate_crumb_cache` 清空。
pub async fn get_crumb(client: &Client) -> Option<String> {
    // 整个「缓存命中检查 → 过期则刷新」都在锁内完成：并发调用者排队，
    // 同一时刻只有一个任务执行握手，其余调用直接拿到同一个 fresh crumb。
    let mut cache = CRUMB_CACHE.lock().await;
    if let Some((c, ts)) = cache.clone() {
        if ts.elapsed().as_secs() < CRUMB_TTL_SECS {
            return Some(c);
        }
        // TTL 过期，清除并重新握手
        *cache = None;
    }
    let crumb = get_crumb_fresh(client).await;
    if crumb.is_some() {
        *cache = crumb.clone().map(|c| (c, Instant::now()));
    }
    crumb
}

async fn get_crumb_fresh(client: &Client) -> Option<String> {
    // 种 cookie（响应状态码无关紧要，忽略结果）
    let _ = client
        .get("https://fc.yahoo.com")
        .header("User-Agent", BROWSER_UA)
        .send()
        .await;

    // getcrumb 偶发 429：退避重试一次
    for attempt in 0..2 {
        let resp = client
            .get("https://query2.finance.yahoo.com/v1/test/getcrumb")
            .header("User-Agent", BROWSER_UA)
            .send()
            .await
            .ok()?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }
        if !resp.status().is_success() {
            return None;
        }
        let crumb = crate::http::text_limited(resp).await.ok()?;
        let crumb = crumb.trim();
        // 校验：crumb 应为短 ASCII 字母数字串；封锁形态下拿到的是 HTML/超长垃圾，
        // 直接视为失败，避免把垃圾串拼进重试 URL 保证失败。
        if !crumb.is_empty()
            && crumb.len() <= 64
            && crumb
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Some(crumb.to_string());
        }
        return None;
    }
    None
}

/// 给 URL 追加 crumb 查询参数（自动判断分隔符 `?` / `&`）。
pub fn append_crumb(url: &str, crumb: &str) -> String {
    let sep = if url.contains('?') { "&" } else { "?" };
    format!("{}{}crumb={}", url, sep, crate::http::url_encode(crumb))
}

/// 检测 Yahoo HTTP 200 + 错误体形态（缺 crumb / 区域封锁的典型响应，
/// 如 `{"finance":{"error":{"code":"Unauthorized"}}}`，或非 JSON 的验证/封锁页）。
fn body_has_yahoo_error(body: &str) -> bool {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(body) else {
        // HTTP 200 但非 JSON（HTML 验证页等）也视为需 crumb 重试
        return true;
    };
    ["finance", "quoteSummary"].iter().any(|k| {
        root.get(k)
            .and_then(|o| o.get("error"))
            .is_some_and(|e| !e.is_null())
    })
}

/// 带 crumb 握手的 Yahoo GET，返回响应 body 文本。
///
/// 先直连（浏览器 UA）；HTTP 层失败（401/403/5xx/传输错误）或
/// HTTP 200 但 body 内嵌 finance/quoteSummary 错误（缺 crumb 的典型形态）
/// 时取 crumb 后重试。chart 端点另有 "HTTP 200 但空数据" 形态，
/// 由 `market/us.rs` 自行处理重试。
pub async fn yahoo_get_body(client: &Client, url: &str) -> Result<String, String> {
    // 第一步：直连（无 crumb）。
    if let Ok(resp) =
        get_with_retry_headers(client, url, &[("User-Agent", BROWSER_UA)], Some(2)).await
    {
        if let Ok(body) = crate::http::text_limited(resp).await {
            if !body_has_yahoo_error(&body) {
                return Ok(body);
            }
        }
    }

    // 第二步：cookie + crumb 握手后重试。
    let crumb = get_crumb(client).await;
    let url2 = match &crumb {
        Some(c) => append_crumb(url, c),
        None => url.to_string(),
    };
    let resp = get_with_retry_headers(client, &url2, &[("User-Agent", BROWSER_UA)], Some(2))
        .await
        .map_err(|e| format!("Yahoo Finance 请求失败: {}", e))?;
    let body = crate::http::text_limited(resp)
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    if body_has_yahoo_error(&body) {
        // 带 crumb 仍返回错误体：crumb 可能已轮换/失效，清除缓存让下一次重新握手
        invalidate_crumb_cache().await;
        return Err("Yahoo Finance 返回错误响应（可能仍被区域封锁）".to_string());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_has_yahoo_error() {
        // finance 错误体（缺 crumb 的典型响应）→ 需重试
        assert!(body_has_yahoo_error(
            r#"{"finance":{"error":{"code":"Unauthorized"}}}"#
        ));
        // quoteSummary 错误体
        assert!(body_has_yahoo_error(
            r#"{"quoteSummary":{"error":{"code":"Not Found"}}}"#
        ));
        // 错误为 null → 正常数据
        assert!(!body_has_yahoo_error(
            r#"{"finance":{"result":[]},"finance.error":null}"#
        ));
        assert!(!body_has_yahoo_error(
            r#"{"quoteSummary":{"result":[{}],"error":null}}"#
        ));
        // 正常 JSON
        assert!(!body_has_yahoo_error(r#"{"foo":1}"#));
        // 非 JSON（HTML 封锁页）→ 需重试
        assert!(body_has_yahoo_error("<!DOCTYPE html><html>captcha</html>"));
    }

    #[test]
    fn test_append_crumb() {
        // 已有查询串 → 用 &
        assert_eq!(
            append_crumb("https://x/y?a=1", "abc"),
            "https://x/y?a=1&crumb=abc"
        );
        // 无查询串 → 用 ?
        assert_eq!(append_crumb("https://x/y", "abc"), "https://x/y?crumb=abc");
        // crumb 含特殊字符需编码
        assert_eq!(
            append_crumb("https://x/y?a=1", "a b&c"),
            "https://x/y?a=1&crumb=a%20b%26c"
        );
    }
}
