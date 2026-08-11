use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

/// 默认请求超时（秒）
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// 默认重试次数
const DEFAULT_RETRIES: u32 = 3;

/// 默认重试间隔（秒）
const DEFAULT_RETRY_DELAY_SECS: u64 = 1;

/// 响应体最大字节数（50 MB）：防止上游异常/被攻破时发送超大响应导致 OOM。
const MAX_RESPONSE_BYTES: usize = 50 * 1024 * 1024;

/// 构建带超时配置的 reqwest HTTP 客户端
///
/// 启用 cookie store：Yahoo Finance 的 crumb 握手需要先种 cookie 再取 crumb，
/// 后续 chart 请求必须带回同一 cookie，否则 crumb 校验失败。
pub fn build_client() -> Result<Client> {
    let client = Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(10))
        .cookie_store(true)
        // 与 Cargo.toml 版本保持一致，避免下次发版时 UA 失同步
        .user_agent(format!("trad-skill/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    Ok(client)
}

/// 带重试的 GET 请求
///
/// 如果请求失败，会按指数退避策略重试，最多重试 `retries` 次。
pub async fn get_with_retry(
    client: &Client,
    url: &str,
    retries: Option<u32>,
) -> Result<reqwest::Response> {
    get_with_retry_headers(client, url, &[], retries).await
}

/// 判断给定 HTTP 状态码是否值得重试。
///
/// 4xx（除 429 限流外）是客户端错误，重试不会改变结果，应立即失败；
/// 5xx 服务端错误与 429 限流才重试。3xx 由 reqwest 自动跟随，不会走到这里。
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    if status.is_client_error() {
        return status.as_u16() == 429;
    }
    true
}

/// 带重试的 GET 请求，每次尝试附加自定义请求头
///
/// 用于需要覆盖默认 User-Agent 的场景（如 Yahoo Finance 需要浏览器 UA）。
/// 仅在 5xx / 429 / 传输层错误时重试；4xx（除 429）立即失败，避免对
/// 401/403/404 等不可恢复错误做无意义退避等待。
pub async fn get_with_retry_headers(
    client: &Client,
    url: &str,
    headers: &[(&str, &str)],
    retries: Option<u32>,
) -> Result<reqwest::Response> {
    let max_retries = retries.unwrap_or(DEFAULT_RETRIES);
    let mut last_err = None;

    for attempt in 0..=max_retries {
        let mut req = client.get(url);
        for (key, value) in headers {
            req = req.header(*key, *value);
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) => {
                let status = resp.status();
                // 429 时优先尊重服务端 Retry-After（秒），避免在限流窗口内继续硬撞
                let retry_after = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    resp.headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                } else {
                    None
                };
                // 读取并丢弃错误响应体（经 text_limited 的 50 MB 上限防护），
                // 让连接归还连接池复用（直接 drop 会关闭连接）
                let _ = text_limited(resp).await;
                let err = anyhow::anyhow!("HTTP 请求失败: {}", status);
                // 4xx（除 429）不可恢复，立即返回，不退避
                if !is_retryable_status(status) {
                    return Err(err);
                }
                last_err = Some(err);
                if attempt < max_retries {
                    let delay = match retry_after {
                        // 上限 30s，避免服务端给超大 Retry-After 时 CLI 长时间挂起
                        Some(secs) => Duration::from_secs(secs.min(30)),
                        None => {
                            Duration::from_secs(DEFAULT_RETRY_DELAY_SECS * 2u64.pow(attempt.min(6)))
                        }
                    };
                    tokio::time::sleep(delay).await;
                }
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!("HTTP 请求异常: {}", e));
                if attempt < max_retries {
                    let delay =
                        Duration::from_secs(DEFAULT_RETRY_DELAY_SECS * 2u64.pow(attempt.min(6)));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("未知请求错误")))
}

/// 简单的百分号 URL 编码（用于查询参数，如 Yahoo crumb、东方财富 JSONP param）。
///
/// 保留 `A-Za-z0-9-_.~`，空格 -> `%20`，其余字节按 `%XX` 编码。
pub fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(*b as char);
            }
            b' ' => result.push_str("%20"),
            _ => result.push_str(&format!("%{:02X}", b)),
        }
    }
    result
}

/// Content-Length 预检：服务端提供 Content-Length 且超过上限时立即拒绝（避免下载）。
///
/// 无 Content-Length（None）时放行，由读取过程中的累计长度检查兜底。
fn check_content_length(len: Option<u64>) -> Result<(), String> {
    if let Some(len) = len {
        if len as usize > MAX_RESPONSE_BYTES {
            return Err(format!(
                "响应体过大（{} 字节，上限 {}）",
                len, MAX_RESPONSE_BYTES
            ));
        }
    }
    Ok(())
}

/// 流式读取响应体原始字节，带大小上限防护（默认 50 MB）。
///
/// 先检查 Content-Length（若服务端提供），超限立即拒绝避免下载；
/// 无 Content-Length 时逐 chunk 读取并累加计数，累计超限立即返回 Err，
/// 而非等整包下载完才检查（避免无 Content-Length 的 chunked 超大响应 OOM）。
/// 返回 `Result<Vec<u8>, String>`（与数据模块错误类型一致）。
pub async fn bytes_limited(resp: reqwest::Response) -> Result<Vec<u8>, String> {
    check_content_length(resp.content_length())?;
    let mut buf = Vec::new();
    let mut total = 0usize;
    let mut resp = resp;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?
    {
        total += chunk.len();
        if total > MAX_RESPONSE_BYTES {
            return Err(format!(
                "响应体过大（{} 字节，上限 {}）",
                total, MAX_RESPONSE_BYTES
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// 读取响应体文本，带大小上限防护（默认 50 MB）。
///
/// 复用 `bytes_limited` 的流式读取与 Content-Length 预检。
/// 返回 `Result<String, String>`（与数据模块错误类型一致）。
pub async fn text_limited(resp: reqwest::Response) -> Result<String, String> {
    let bytes = bytes_limited(resp).await?;
    String::from_utf8(bytes).map_err(|e| format!("响应体非有效 UTF-8: {}", e))
}

/// 读取响应体并解析为 JSON，带大小上限防护（复用 `text_limited` 的 50 MB 上限）。
pub async fn json_limited(resp: reqwest::Response) -> Result<serde_json::Value, String> {
    let body = text_limited(resp).await?;
    serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("AAPL"), "AAPL");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(url_encode("a+b/c"), "a%2Bb%2Fc");
        // 非 ASCII（中文）按 UTF-8 字节编码
        assert_eq!(url_encode("腾讯"), "%E8%85%BE%E8%AE%AF");
    }

    #[test]
    fn test_check_content_length_over_limit() {
        let err = check_content_length(Some(MAX_RESPONSE_BYTES as u64 + 1)).unwrap_err();
        assert!(err.contains("响应体过大"), "超限应报响应体过大: {}", err);
    }

    #[test]
    fn test_check_content_length_within_limit() {
        assert!(check_content_length(Some(1024)).is_ok());
    }

    #[test]
    fn test_check_content_length_none() {
        // 无 Content-Length 时放行，由读取过程中的累计长度检查兜底
        assert!(check_content_length(None).is_ok());
    }

    #[test]
    fn test_is_retryable_status() {
        use reqwest::StatusCode;
        // 4xx（除 429）不可重试
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST)); // 400
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED)); // 401
        assert!(!is_retryable_status(StatusCode::FORBIDDEN)); // 403
        assert!(!is_retryable_status(StatusCode::NOT_FOUND)); // 404
                                                              // 429 限流可重试
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        // 5xx 服务端错误可重试
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR)); // 500
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY)); // 502
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE)); // 503
    }

    // ── 流式限流测试：构造无 Content-Length 的 chunked 响应，证明限流在
    //    流式读取过程中触发（而非整包下载完才检查）──

    /// 测试用流式响应体：按序产出给定 chunks，耗尽后永久挂起（不结束流）。
    ///
    /// 模拟未知总长度的 chunked 传输 —— 若读取方用 `resp.bytes()` 整包下载，
    /// 将永远等不到流结束（在测试里体现为 tokio timeout 失败），从而证明
    /// 限流必须发生在逐 chunk 读取过程中。
    ///
    /// `size_hint`：reqwest 的 `Response::content_length()` 读的是 body 的
    /// `size_hint().exact()`（真实 hyper Incoming 由线上 Content-Length 推导），
    /// 因此带长度声明的测试响应必须在这里同步提供 SizeHint。
    struct ChunkedBody {
        chunks: std::collections::VecDeque<hyper::body::Bytes>,
        size_hint: Option<u64>,
    }

    impl ChunkedBody {
        fn new(chunks: Vec<hyper::body::Bytes>, size_hint: Option<u64>) -> Self {
            Self {
                chunks: chunks.into(),
                size_hint,
            }
        }
    }

    impl hyper::body::Body for ChunkedBody {
        type Data = hyper::body::Bytes;
        type Error = std::io::Error;

        fn poll_frame(
            mut self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
            match self.chunks.pop_front() {
                Some(chunk) => std::task::Poll::Ready(Some(Ok(hyper::body::Frame::data(chunk)))),
                // 流永不完结：挂起且不注册 waker —— 整包下载实现会在此永久等待
                None => std::task::Poll::Pending,
            }
        }

        fn size_hint(&self) -> hyper::body::SizeHint {
            let mut hint = hyper::body::SizeHint::new();
            if let Some(len) = self.size_hint {
                hint.set_exact(len);
            }
            hint
        }
    }

    /// 包一层自定义流式响应体，构造不经过真实网络的 reqwest::Response。
    /// `content_length = None` 模拟无 Content-Length 的 chunked 响应。
    fn response_with_body(body: reqwest::Body, content_length: Option<u64>) -> reqwest::Response {
        let mut builder = hyper::http::Response::builder().status(reqwest::StatusCode::OK);
        if let Some(len) = content_length {
            builder = builder.header(hyper::http::header::CONTENT_LENGTH, len);
        }
        let http_resp = builder.body(body).expect("构造测试响应失败");
        reqwest::Response::from(http_resp)
    }

    /// 构造指定 chunk 大小/数量的永不完结 chunked 响应。
    /// `content_length` 同时写入响应头与 body 的 SizeHint（对齐真实 hyper 行为）。
    fn chunked_response(
        chunk_size: usize,
        chunk_count: usize,
        content_length: Option<u64>,
    ) -> reqwest::Response {
        let chunk: hyper::body::Bytes = vec![0u8; chunk_size].into();
        response_with_body(
            reqwest::Body::wrap(ChunkedBody::new(vec![chunk; chunk_count], content_length)),
            content_length,
        )
    }

    #[tokio::test]
    async fn test_bytes_limited_stops_at_cap() {
        // 3 chunks × 20MB = 60MB，MAX=50MB：前 2 个累加 40MB，第 3 个触发 Err。
        // 响应体永不完结：若先整包下载再检查（resp.bytes()），本测试会在
        // timeout 后失败 —— 证明限流发生在流式读取过程中，且错误信息里的
        // 累计字节数（62914560 = 3×20MB）证明是第 3 个 chunk 触发的。
        let resp = chunked_response(20 * 1024 * 1024, 3, None);
        let err = tokio::time::timeout(Duration::from_secs(10), bytes_limited(resp))
            .await
            .expect("bytes_limited 应在流式过程中返回，而非挂起等待流结束")
            .expect_err("超过 50MB 上限应返回 Err");
        assert!(
            err.contains("62914560"),
            "应在第 3 个 chunk（累计 60MB）触发: {}",
            err
        );
        assert!(err.contains("响应体过大"), "超限应报响应体过大: {}", err);
    }

    #[tokio::test]
    async fn test_text_limited_rejects_oversized_chunked() {
        // chunked transfer、无 Content-Length、共 100MB 的流：应返回 Err 而非 OOM。
        // 上限 50MB 在第 3 个 chunk（累计 60MB）即触发，不会读完全部 100MB。
        // 流永不完结 —— 旧实现（resp.bytes() 全量读取）会挂起直到 timeout。
        let resp = chunked_response(20 * 1024 * 1024, 5, None);
        let err = tokio::time::timeout(Duration::from_secs(10), text_limited(resp))
            .await
            .expect("text_limited 应在流式过程中返回，而非挂起等待流结束")
            .expect_err("超过 50MB 上限应返回 Err");
        assert!(
            err.contains("62914560"),
            "应在第 3 个 chunk（累计 60MB）触发: {}",
            err
        );
        assert!(err.contains("响应体过大"), "超限应报响应体过大: {}", err);
    }

    #[tokio::test]
    async fn test_bytes_limited_content_length_precheck() {
        // Content-Length 预检保留：size_hint 超限立即拒绝，不进入流式读取
        let resp = chunked_response(1024, 1, Some(MAX_RESPONSE_BYTES as u64 + 1));
        let err = tokio::time::timeout(Duration::from_secs(10), bytes_limited(resp))
            .await
            .expect("bytes_limited 应预检拒绝，而非挂起等待流结束")
            .expect_err("Content-Length 超限应预检拒绝");
        assert!(err.contains("响应体过大"), "超限应报响应体过大: {}", err);
    }
}
