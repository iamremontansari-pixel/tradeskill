/// 新闻数据获取模块
///
/// 支持多数据源新闻获取：
/// - Yahoo Finance 新闻（美股）
/// - Google News RSS（全球）
/// - 东方财富新闻（A股）
///
/// 对齐 Python fetch_news.py 的输出格式。
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::Client;
use serde_json::Value;

use crate::http::get_with_retry;
use crate::market::{detect_market, Market};
use crate::yahoo::yahoo_get_body;

/// 摘要最大字符数
const MAX_SUMMARY_CHARS: usize = 200;

// ───────────────────────── 工具函数 ─────────────────────────

/// 截断文本到指定长度
fn truncate(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() > limit {
        let truncated: String = text.chars().take(limit).collect();
        format!("{}...", truncated)
    } else {
        text.to_string()
    }
}

/// 格式化单条新闻为 markdown
fn format_news_item(title: &str, source: &str, summary: &str) -> String {
    let title = if title.is_empty() { "无标题" } else { title };
    let mut lines = vec![format!("- **{}**", title)];
    let mut parts = Vec::new();
    if !source.is_empty() {
        parts.push(format!("来源:{}", source));
    }
    if !summary.is_empty() {
        parts.push(truncate(summary, MAX_SUMMARY_CHARS));
    }
    if !parts.is_empty() {
        lines.push(format!("  - {}", parts.join(" | ")));
    }
    lines.join("\n")
}

/// 清除 HTML 标签（简单实现：移除 <...> 内容）并解码常见 HTML 实体。
///
/// 顺序：先剥标签，再解码实体。若先解码会把 `&lt;`/`&gt;` 还原成 `<`/`>` 而
/// 被剥标签逻辑误删。支持命名实体（`&amp;`/`&lt;`/`&gt;`/`&quot;`/`&apos;`/`&nbsp;`）
/// 与数字实体（`&#39;`/`&#x27;`），未知实体原样保留。
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    decode_html_entities(result.trim())
}

/// 解码常见 HTML 实体。`&` 后 15 个字符内无 `;` 视作字面 `&`。
fn decode_html_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            // 在合理窗口内寻找 ';'
            let tail = &s[i..];
            if let Some(rel) = tail.find(';') {
                if rel <= 15 {
                    let entity = &tail[1..rel];
                    if let Some(decoded) = decode_one_entity(entity) {
                        out.push_str(&decoded);
                        i += rel + 1;
                        continue;
                    }
                }
            }
            out.push('&');
            i += 1;
        } else {
            // i 恒在字符边界（按 len_utf8 步进），unwrap_or 仅作防御
            let ch = s[i..].chars().next().unwrap_or('\u{FFFD}');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// 解码单个实体（不含首尾的 `&` / `;`）。未知返回 None。
fn decode_one_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => Some("&".to_string()),
        "lt" => Some("<".to_string()),
        "gt" => Some(">".to_string()),
        "quot" => Some("\"".to_string()),
        "apos" => Some("'".to_string()),
        "nbsp" => Some(" ".to_string()),
        _ => {
            let num = entity.strip_prefix('#')?;
            let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                num.parse::<u32>().ok()
            };
            code.and_then(char::from_u32).map(|c| c.to_string())
        }
    }
}

// ───────────────────────── Yahoo Finance 新闻 ─────────────────────────

/// 通过 Yahoo Finance v10 API 获取个股新闻
async fn fetch_yfinance_news(client: &Client, symbol: &str, days: u32, limit: u32) -> String {
    let days = days.max(1);
    let url = format!(
        "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=news",
        crate::http::url_encode(symbol)
    );

    let resp_text = match yahoo_get_body(client, &url).await {
        Ok(b) => b,
        Err(e) => return format!("错误: 获取 {} 新闻失败 - {}", symbol, e),
    };

    let body: Value = match serde_json::from_str(&resp_text) {
        Ok(v) => v,
        Err(e) => return format!("错误: 解析 {} 新闻响应失败 - {}", symbol, e),
    };

    // 提取新闻数组
    let news_arr = match body
        .get("quoteSummary")
        .and_then(|q| q.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("news"))
        .and_then(|n| n.as_array())
    {
        Some(arr) if !arr.is_empty() => arr,
        _ => return format!("未找到 {} 的相关新闻。", symbol),
    };

    // 按时间过滤并收集结果
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let mut results = Vec::new();

    for item in news_arr {
        if results.len() >= limit as usize {
            break;
        }

        // 尝试从 content 嵌套结构或顶层提取字段
        let content_obj = item.get("content").and_then(|c| c.as_object());
        let title = content_obj
            .and_then(|c| c.get("title"))
            .and_then(|v| v.as_str())
            .or_else(|| item.get("title").and_then(|v| v.as_str()))
            .unwrap_or("无标题");

        // 提取来源
        let source = content_obj
            .and_then(|c| c.get("provider"))
            .and_then(|p| p.get("displayName"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 提取摘要
        let summary = content_obj
            .and_then(|c| c.get("summary").or_else(|| c.get("description")))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 时间过滤：尝试解析发布时间（content 嵌套与顶层字段都查，借用不克隆）
        let pub_time = parse_news_time(item);
        if let Some(pt) = pub_time {
            if pt < cutoff {
                continue;
            }
        }
        // 解析失败的条目保留（避免字段变更导致漏报）

        results.push(format_news_item(title, source, summary));
    }

    if results.is_empty() {
        return format!("未找到 {} 在最近 {} 天内的相关新闻。", symbol, days);
    }

    let header = format!(
        "## {} 相关新闻（最近 {} 天，共 {} 条）\n\n",
        symbol,
        days,
        results.len()
    );
    format!("{}{}", header, results.join("\n"))
}

/// 解析新闻发布时间。
///
/// 字段可能嵌套在 `content` 里（Yahoo v10 结构），也可能在顶层：
/// 两者都查，只借用不克隆 content 对象。`None` 表示字段缺失或解析失败
/// （调用方对 None 的条目保留，避免字段变更导致漏报）。
fn parse_news_time(item: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    let content = item.get("content");
    // 尝试 ISO 8601 字符串
    for field in ["pubDate", "publishTime"] {
        let s = content
            .and_then(|c| c.get(field))
            .or_else(|| item.get(field))
            .and_then(|v| v.as_str());
        if let Some(s) = s {
            // 尝试 ISO 格式
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                return Some(dt.with_timezone(&chrono::Utc));
            }
            // 尝试常见格式
            if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ") {
                return Some(dt.with_timezone(&chrono::Utc));
            }
        }
    }
    // 尝试 Unix 时间戳
    for field in ["providerPublishTime", "pubDate", "publishTime"] {
        let n = content
            .and_then(|c| c.get(field))
            .or_else(|| item.get(field))
            .and_then(|v| v.as_f64());
        if let Some(n) = n {
            let ts = if n > 1e12 {
                (n / 1000.0) as i64
            } else {
                n as i64
            };
            if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                return Some(dt);
            }
        }
    }
    None
}

// ───────────────────────── Google News RSS ─────────────────────────

/// 通过 Google News RSS 获取新闻
async fn fetch_google_news(
    client: &Client,
    query: &str,
    days: u32,
    limit: u32,
    lang: &str,
) -> String {
    let days = days.max(1);
    let encoded_query = crate::http::url_encode(query);
    let (hl, gl, ceid) = match lang {
        "zh" => ("zh-CN", "CN", "CN:zh-Hans"),
        _ => ("en", "US", "US:en"),
    };
    let url = format!(
        "https://news.google.com/rss/search?q={}+when:{}d&hl={}&gl={}&ceid={}",
        encoded_query, days, hl, gl, ceid
    );

    let resp = match get_with_retry(client, &url, Some(2)).await {
        Ok(r) => r,
        Err(e) => return format!("错误: Google News 请求失败 - {}", e),
    };

    let xml_text = match crate::http::text_limited(resp).await {
        Ok(t) => t,
        Err(e) => return format!("错误: 读取 Google News 响应失败 - {}", e),
    };

    // 解析 RSS XML
    parse_google_news_rss(&xml_text, query, days, limit)
}

/// 解析 Google News RSS XML
fn parse_google_news_rss(xml: &str, query: &str, days: u32, limit: u32) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut in_item = false;
    let mut current_title = String::new();
    let mut current_source = String::new();
    let mut current_desc = String::new();
    let mut current_tag = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                // 标签名按字节切片比较，避免每个标签一次 String 分配
                match e.name().as_ref() {
                    b"item" => {
                        in_item = true;
                        current_title.clear();
                        current_source.clear();
                        current_desc.clear();
                    }
                    b"title" if in_item => current_tag = "title".to_string(),
                    b"source" if in_item => current_tag = "source".to_string(),
                    b"description" if in_item => current_tag = "description".to_string(),
                    _ => current_tag.clear(),
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" => current_title.push_str(&text),
                    "source" => current_source.push_str(&text),
                    "description" => current_desc.push_str(&text),
                    _ => {}
                }
            }
            // Google News 把 <description> 包在 CDATA 里，quick-xml 产生 Event::CData。
            // CDATA 内容是原始文本（实体不在此层转义，`&amp;`/`&nbsp;` 等由下游 strip_html
            // 的 decode_html_entities 解码），直接按 UTF-8 取字节即可。
            Ok(Event::CData(ref e)) => {
                let text = String::from_utf8_lossy(e).into_owned();
                match current_tag.as_str() {
                    "title" => current_title.push_str(&text),
                    "source" => current_source.push_str(&text),
                    "description" => current_desc.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"item" {
                    in_item = false;
                    if !current_title.is_empty() {
                        items.push(format_news_item(
                            &current_title,
                            &current_source,
                            &strip_html(&current_desc),
                        ));
                    }
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return format!("错误: 解析 Google News RSS 响应失败 - {}", e),
            _ => {}
        }
    }

    if items.is_empty() {
        return format!("Google News 未找到与 \"{}\" 相关的新闻。", query);
    }

    let display_items: Vec<_> = items.into_iter().take(limit as usize).collect();
    let header = format!(
        "## Google News: \"{}\"（最近 {} 天，共 {} 条）\n\n",
        query,
        days,
        display_items.len()
    );
    format!("{}{}", header, display_items.join("\n"))
}

// ───────────────────────── A股新闻（东方财富）─────────────────────────

/// 解析东方财富文章发布时间（如 `"2026-07-30 21:25:00"`）。
///
/// 东方财富日期为北京时间（UTC+8），这里按 UTC 解析；`days` 级别过滤下 8h 时差可忽略。
/// 支持 `YYYY-MM-DD HH:MM:SS` 与 `YYYY-MM-DD HH:MM` 两种格式；解析失败返回 None
/// （调用方对 None 的条目保留，避免字段变更导致漏报）。
fn parse_eastmoney_article_date(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = s.trim();
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .or_else(|| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").ok())
        .map(|ndt| ndt.and_utc())
}

/// 从东方财富搜索响应中提取文章，按 `days` 过滤后取至多 `limit` 条格式化结果。
///
/// - 早于 `now - days` 的文章跳过（`--days` 过滤）；无 `date` 或解析失败的条目保留。
/// - 无 `cmsArticleWebOld` 数组或数组为空时返回 None（由调用方降级到 Google News）。
fn filter_eastmoney_articles(body: &Value, days: u32, limit: u32) -> Option<Vec<String>> {
    let arr = body
        .get("result")
        .and_then(|r| r.get("cmsArticleWebOld"))
        .and_then(|a| a.as_array())?;
    if arr.is_empty() {
        return None;
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let mut results = Vec::new();
    for article in arr {
        if results.len() >= limit as usize {
            break;
        }
        if let Some(s) = article.get("date").and_then(|v| v.as_str()) {
            if let Some(pt) = parse_eastmoney_article_date(s) {
                if pt < cutoff {
                    continue;
                }
            }
        }
        let title = strip_html(
            article
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("无标题"),
        );
        let source = article
            .get("mediaName")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let content = strip_html(
            article
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        );
        results.push(format_news_item(&title, source, &content));
    }
    Some(results)
}

/// 通过东方财富搜索 API 获取A股/港股新闻。
/// `code` 为搜索关键字（A股=6位代码，港股=5位零填充代码），`tag` 用于标题与降级查询（"A股"/"港股"）。
///
/// `sort=time` 按发布时间倒序拉取最新文章，再按 `days` 客户端过滤，使 `--days` 真正生效
/// （此前 `sort=default` + 无日期过滤，`--days` 被忽略，常返回数周前旧闻）。
async fn fetch_eastmoney_news(
    client: &Client,
    code: &str,
    days: u32,
    limit: u32,
    tag: &str,
) -> String {
    let days = days.max(1);
    // 候选池取 max(limit, 30)：sort=time 拿最新文章，过滤后仍能凑够 limit 条。
    let param = serde_json::json!({
        "uid": "",
        "keyword": code,
        "type": ["cmsArticleWebOld"],
        "client": "web",
        "clientType": "web",
        "clientVersion": "curr",
        "param": {
            "cmsArticleWebOld": {
                "searchScope": "default",
                "sort": "time",
                "pageIndex": 1,
                "pageSize": limit.max(30),
                "preTag": "",
                "postTag": ""
            }
        }
    });

    let url = format!(
        "https://search-api-web.eastmoney.com/search/jsonp?cb=jQuery&param={}",
        crate::http::url_encode(&param.to_string())
    );

    // 取数 + 解析任一步失败均降级到 Google News 中文（其自身按 when:days 过滤）。
    let em_items = match get_with_retry(client, &url, Some(2)).await {
        Ok(resp) => match crate::http::text_limited(resp).await {
            Ok(text) => {
                let json_str = strip_jsonp(&text);
                match serde_json::from_str::<Value>(&json_str) {
                    Ok(body) => filter_eastmoney_articles(&body, days, limit),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        },
        Err(_) => None,
    };

    if let Some(items) = em_items {
        if !items.is_empty() {
            let header = format!(
                "## {} {} 相关新闻（最近 {} 天，共 {} 条）\n\n",
                tag,
                code,
                days,
                items.len()
            );
            return format!("{}{}", header, items.join("\n"));
        }
    }

    // 东方财富无结果（或全部被日期过滤），降级到 Google News 中文
    fetch_google_news(client, &format!("{} {}", code, tag), days, limit, "zh").await
}

/// 剥离 JSONP 包装：jQuery...({...}) → {...}
fn strip_jsonp(text: &str) -> String {
    let trimmed = text.trim();
    // 找到第一个 '(' 和最后一个 ')'
    if let (Some(start), Some(end)) = (trimmed.find('('), trimmed.rfind(')')) {
        if start < end {
            return trimmed[start + 1..end].to_string();
        }
    }
    trimmed.to_string()
}

// ───────────────────────── 统一入口 ─────────────────────────

/// 统一新闻获取入口
///
/// 自动检测市场类型：
/// - A股（6位纯数字）→ 东方财富新闻，失败降级 Google News 中文
/// - 港股（.HK / 5位数字）→ 东方财富新闻（5位代码），失败降级 Google News 中文
/// - 其他 → Yahoo Finance 新闻 + Google News，并行获取后合并输出
///
/// 契约：永不 panic，错误以 Err(String) 返回（调用方不再靠字符串前缀探测错误）。
pub async fn fetch_news(
    client: &Client,
    symbol: &str,
    days: u32,
    limit: u32,
) -> Result<String, String> {
    let out = fetch_news_inner(client, symbol, days, limit).await;
    if out.starts_with("错误") {
        Err(out)
    } else {
        Ok(out)
    }
}

async fn fetch_news_inner(client: &Client, symbol: &str, days: u32, limit: u32) -> String {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        return "错误: 股票代码不能为空".to_string();
    }

    let market = detect_market(symbol);
    match market {
        Market::CNStock => fetch_eastmoney_news(client, symbol, days, limit, "A股").await,
        Market::HKStock => {
            // 港股用 5 位零填充代码作为东方财富搜索关键字
            let code = crate::market::hk_eastmoney_code(symbol);
            fetch_eastmoney_news(client, &code, days, limit, "港股").await
        }
        _ => {
            // 美股/加密：并行获取 Yahoo Finance + Google News
            let (yf_news, google_news) = tokio::join!(
                fetch_yfinance_news(client, symbol, days, limit),
                fetch_google_news(client, symbol, days, limit, "en")
            );

            let yf_ok = !yf_news.starts_with("错误");
            let g_ok = !google_news.starts_with("错误");
            if !yf_ok && !g_ok {
                // 两个源都失败：返回 Yahoo 的错误信息
                return yf_news;
            }
            let mut sections = Vec::new();
            if yf_ok {
                sections.push(yf_news);
            }
            if g_ok {
                sections.push(google_news);
            }
            sections.join("\n---\n\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<b>hello</b>"), "hello");
        assert_eq!(strip_html("<p>text</p>"), "text");
        assert_eq!(strip_html("no tags"), "no tags");
        assert_eq!(strip_html("<a href='url'>link</a>"), "link");
        assert_eq!(strip_html(""), "");
        // HTML 实体解码
        assert_eq!(strip_html("a &amp; b"), "a & b");
        assert_eq!(strip_html("price &lt; 5 &gt; 3"), "price < 5 > 3");
        assert_eq!(strip_html("it&#39;s"), "it's");
        assert_eq!(strip_html("a&nbsp;b"), "a b");
        assert_eq!(strip_html("<b>A&amp;B</b>"), "A&B");
        // &lt;/&gt; 在标签外应解码为字面 < >，而非被当标签删除
        assert_eq!(strip_html("&lt;not a tag&gt;"), "<not a tag>");
        // 字面 &（非实体）与未知实体保留
        assert_eq!(strip_html("Tom & Jerry"), "Tom & Jerry");
        assert_eq!(strip_html("foo &bar; baz"), "foo &bar; baz");
    }

    #[test]
    fn test_strip_jsonp() {
        assert_eq!(strip_jsonp("jQuery({\"a\":1})"), "{\"a\":1}");
        assert_eq!(strip_jsonp("callback(data)"), "data");
        assert_eq!(strip_jsonp("noparens"), "noparens");
        assert_eq!(strip_jsonp(""), "");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        let long = "a".repeat(300);
        let result = truncate(&long, 200);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 203); // 200 + "..."
    }

    #[test]
    fn test_format_news_item() {
        let item = format_news_item("Title", "Source", "Summary text");
        assert!(item.contains("**Title**"));
        assert!(item.contains("Source"));
        assert!(item.contains("Summary text"));
    }

    // Google News RSS 的 <description> 包裹在 CDATA 中，quick-xml 产生 Event::CData 而非
    // Event::Text。修复前 CDATA 内容被忽略、description 为空（summary 缺失）。
    #[test]
    fn test_parse_google_news_cdata_description() {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>Google News</title>
<item>
<title>Apple stock rises on earnings beat</title>
<source url="https://www.example.com">Example News</source>
<description><![CDATA[<a href="https://www.example.com/apple">Apple &amp; Google shares climbed</a>&nbsp;&nbsp;<font color="#6f6f6f">Example News</font>]]></description>
<pubDate>Tue, 11 Aug 2026 10:00:00 GMT</pubDate>
</item>
</channel></rss>"##;
        let out = parse_google_news_rss(xml, "AAPL", 7, 8);
        // CDATA 内容应进入 description，剥标签 + 解码实体后含链接文本
        assert!(
            out.contains("Apple & Google shares climbed"),
            "CDATA 描述应被解析: {}",
            out
        );
        assert!(
            out.contains("Apple stock rises on earnings beat"),
            "标题应保留: {}",
            out
        );
    }

    // 非 CDATA 的纯文本 <description> 仍应可解析（回归保护）。
    #[test]
    fn test_parse_google_news_text_description() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>Google News</title>
<item>
<title>Apple stock rises</title>
<source url="https://www.example.com">Example News</source>
<description>Apple shares climbed on strong earnings</description>
</item>
</channel></rss>"#;
        let out = parse_google_news_rss(xml, "AAPL", 7, 8);
        assert!(
            out.contains("Apple shares climbed"),
            "Text 描述应被解析: {}",
            out
        );
        assert!(out.contains("Apple stock rises"), "标题应保留: {}", out);
    }

    #[test]
    fn test_parse_eastmoney_article_date() {
        // 标准格式
        let dt = parse_eastmoney_article_date("2026-07-30 21:25:00").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-07-30");
        // 无秒格式
        assert!(parse_eastmoney_article_date("2026-07-30 21:25").is_some());
        // 带空白
        assert!(parse_eastmoney_article_date("  2026-07-30 21:25:00  ").is_some());
        // 无效 / 空
        assert!(parse_eastmoney_article_date("not a date").is_none());
        assert!(parse_eastmoney_article_date("").is_none());
    }

    // 验证 --days 过滤：7 天内的保留，30 天前的剔除（修复前 days 被完全忽略）。
    #[test]
    fn test_filter_eastmoney_articles_days_filter() {
        use serde_json::json;
        let now = chrono::Utc::now();
        let recent = (now - chrono::Duration::days(1))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let old = (now - chrono::Duration::days(30))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let body = json!({
            "result": {
                "cmsArticleWebOld": [
                    {"title": "recent", "date": recent, "mediaName": "src", "content": "c1"},
                    {"title": "old", "date": old, "mediaName": "src", "content": "c2"},
                ]
            }
        });
        let items = filter_eastmoney_articles(&body, 7, 10).unwrap();
        assert_eq!(items.len(), 1, "应只保留 7 天内的文章: {:?}", items);
        assert!(items[0].contains("recent"));
    }

    // 无 date 字段的条目应保留（避免字段变更导致漏报）。
    #[test]
    fn test_filter_eastmoney_articles_keeps_no_date() {
        use serde_json::json;
        let body = json!({
            "result": {
                "cmsArticleWebOld": [
                    {"title": "noDate", "mediaName": "src", "content": "c"},
                ]
            }
        });
        let items = filter_eastmoney_articles(&body, 7, 10).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].contains("noDate"));
    }

    // limit 应截断结果条数。
    #[test]
    fn test_filter_eastmoney_articles_limit() {
        use serde_json::json;
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let body = json!({
            "result": {
                "cmsArticleWebOld": [
                    {"title": "a", "date": now, "mediaName": "", "content": ""},
                    {"title": "b", "date": now, "mediaName": "", "content": ""},
                    {"title": "c", "date": now, "mediaName": "", "content": ""},
                ]
            }
        });
        let items = filter_eastmoney_articles(&body, 7, 2).unwrap();
        assert_eq!(items.len(), 2, "应受 limit 截断");
    }

    // 空数组 / 无字段 -> None（调用方降级到 Google News）。
    #[test]
    fn test_filter_eastmoney_articles_empty() {
        use serde_json::json;
        let empty = json!({"result": {"cmsArticleWebOld": []}});
        assert!(filter_eastmoney_articles(&empty, 7, 10).is_none());
        let missing = json!({"result": {}});
        assert!(filter_eastmoney_articles(&missing, 7, 10).is_none());
    }
}
