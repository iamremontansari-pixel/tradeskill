/// 市场情绪数据获取模块
///
/// 支持多数据源情绪数据获取：
/// - StockTwits（美股社交情绪）
/// - Reddit（美股社区讨论）
/// - 东方财富千股千评 / 机构参与度（A股）
///
/// 对齐 Python fetch_sentiment.py 的输出格式。
use reqwest::Client;
use serde_json::Value;

use crate::http::get_with_retry;
use crate::market::{detect_market, Market};

/// 最近消息展示条数
const RECENT_MESSAGE_DISPLAY: usize = 8;
/// Reddit 帖子展示条数
const REDDIT_POST_DISPLAY: usize = 8;

// ───────────────────────── StockTwits ─────────────────────────

/// 获取 StockTwits 情绪数据
async fn fetch_stocktwits(client: &Client, symbol: &str, limit: u32) -> String {
    let url = format!(
        "https://api.stocktwits.com/api/2/streams/symbol/{}.json",
        crate::http::url_encode(symbol)
    );

    let resp = match get_with_retry(client, &url, Some(1)).await {
        Ok(r) => r,
        Err(_) => return "<unavailable>".to_string(),
    };

    let data: Value = match crate::http::json_limited(resp).await {
        Ok(v) => v,
        Err(_) => return "<unavailable>".to_string(),
    };

    let messages = match data.get("messages").and_then(|m| m.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return format!("# StockTwits 情绪 ({})\n\n> 无消息数据\n", symbol),
    };

    let mut bullish_count = 0u32;
    let mut bearish_count = 0u32;
    let mut neutral_count = 0u32;
    let mut recent_messages = Vec::new();

    let display_limit = limit.min(messages.len() as u32) as usize;
    for msg in messages.iter().take(display_limit) {
        // 提取情绪标签
        let basic = msg
            .get("entities")
            .and_then(|e| e.get("sentiment"))
            .and_then(|s| s.get("basic"))
            .and_then(|b| b.as_str())
            .unwrap_or("");

        match basic {
            "Bullish" => bullish_count += 1,
            "Bearish" => bearish_count += 1,
            _ => neutral_count += 1,
        }

        // 提取消息体
        if let Some(body) = msg.get("body").and_then(|b| b.as_str()) {
            if !body.is_empty() {
                let truncated = if body.chars().count() > 200 {
                    let t: String = body.chars().take(200).collect();
                    format!("{}...", t)
                } else {
                    body.to_string()
                };
                recent_messages.push(format!("- {}", truncated));
            }
        }
    }

    // 计算看涨/看跌比例
    let total_tagged = bullish_count + bearish_count;
    let (bullish_pct, bearish_pct) = if total_tagged > 0 {
        (
            (bullish_count as f64 / total_tagged as f64 * 100.0 * 10.0).round() / 10.0,
            (bearish_count as f64 / total_tagged as f64 * 100.0 * 10.0).round() / 10.0,
        )
    } else {
        (0.0, 0.0)
    };

    let mut sections = Vec::new();
    sections.push(format!("# StockTwits 情绪 ({})\n", symbol));
    sections.push("## 情绪统计\n".to_string());
    sections.push(format!(
        "- **看涨 (Bullish)**: {} ({:.1}%)",
        bullish_count, bullish_pct
    ));
    sections.push(format!(
        "- **看跌 (Bearish)**: {} ({:.1}%)",
        bearish_count, bearish_pct
    ));
    sections.push(format!("- **中性/未标注**: {}", neutral_count));
    sections.push(String::new());
    sections.push("## 最近消息\n".to_string());
    for msg in recent_messages.iter().take(RECENT_MESSAGE_DISPLAY) {
        sections.push(msg.clone());
    }

    sections.join("\n")
}

// ───────────────────────── Reddit ─────────────────────────

/// 获取单个 subreddit 的搜索结果
async fn fetch_subreddit(
    client: &Client,
    subreddit: &str,
    symbol: &str,
    time_filter: &str,
) -> Vec<RedditPost> {
    let url = format!(
        "https://www.reddit.com/r/{}/search.json?q={}&sort=new&t={}&limit=10",
        subreddit,
        crate::http::url_encode(symbol),
        time_filter
    );

    // Reddit 对通用 UA 越来越严，且偶发 429：走共享重试 + 浏览器 UA。
    let resp = match crate::http::get_with_retry_headers(
        client,
        &url,
        &[("User-Agent", crate::yahoo::BROWSER_UA)],
        Some(1),
    )
    .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return Vec::new(),
    };

    let data: Value = match crate::http::json_limited(resp).await {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut posts = Vec::new();
    if let Some(children) = data
        .get("data")
        .and_then(|d| d.get("children"))
        .and_then(|c| c.as_array())
    {
        for child in children {
            if let Some(post_data) = child.get("data") {
                posts.push(RedditPost {
                    title: post_data
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    score: post_data
                        .get("score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as i64,
                    num_comments: post_data
                        .get("num_comments")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as i64,
                    subreddit: subreddit.to_string(),
                });
            }
        }
    }
    posts
}

/// 获取 Reddit 情绪数据
async fn fetch_reddit_sentiment(client: &Client, symbol: &str, days: u32) -> String {
    let days = days.max(1);
    let time_filter = if days <= 7 { "week" } else { "month" };

    // 并行获取 3 个 subreddit
    let (wsb, stocks, investing) = tokio::join!(
        fetch_subreddit(client, "wallstreetbets", symbol, time_filter),
        fetch_subreddit(client, "stocks", symbol, time_filter),
        fetch_subreddit(client, "investing", symbol, time_filter)
    );

    let mut all_posts: Vec<RedditPost> = Vec::new();
    all_posts.extend(wsb);
    all_posts.extend(stocks);
    all_posts.extend(investing);

    if all_posts.is_empty() {
        return "<unavailable>".to_string();
    }

    // 按互动量（分数 + 评论数）降序排序
    all_posts.sort_by_key(|p| std::cmp::Reverse(p.score + p.num_comments));

    let mut sections = Vec::new();
    sections.push(format!("# Reddit 情绪 ({})\n", symbol));
    sections.push(format!(
        "搜索范围: r/wallstreetbets, r/stocks, r/investing（最近 {} 天）\n",
        days
    ));
    sections.push("## 热门帖子（按互动量排序）\n".to_string());

    for post in all_posts.iter().take(REDDIT_POST_DISPLAY) {
        let title = if post.title.chars().count() > 100 {
            let t: String = post.title.chars().take(100).collect();
            format!("{}...", t)
        } else {
            post.title.clone()
        };
        sections.push(format!(
            "- [{}] {} (⬆{} | 💬{})",
            post.subreddit, title, post.score, post.num_comments
        ));
    }

    sections.join("\n")
}

/// Reddit 帖子数据结构
struct RedditPost {
    title: String,
    score: i64,
    num_comments: i64,
    subreddit: String,
}

// ───────────────────────── A股情绪 ─────────────────────────

/// 获取A股情绪数据（东方财富 API）
async fn fetch_cn_sentiment(client: &Client, symbol: &str) -> String {
    let mut sections = Vec::new();
    sections.push(format!("# A股情绪分析 ({})\n", symbol));

    let mut has_data = false;

    // ── 千股千评（stock_comment_em 对应接口）──
    // 东方财富千股千评 API
    let comment_url = format!(
        "https://datacenter-web.eastmoney.com/api/data/v1/get?sortColumns=SECURITY_CODE&sortTypes=1&pageSize=500&pageNumber=1&reportName=RPT_DMSK_TS_STOCKNEW&columns=ALL&token=894050c76af8597a853f5b408b759f5d&filter=(SECURITY_CODE=%22{}%22)",
        symbol
    );

    match get_with_retry(client, &comment_url, Some(2)).await {
        Ok(resp) => {
            if let Ok(body) = crate::http::json_limited(resp).await {
                let data_arr = body
                    .get("result")
                    .and_then(|r| r.get("data"))
                    .and_then(|d| d.as_array());

                if let Some(arr) = data_arr {
                    if !arr.is_empty() {
                        has_data = true;
                        sections.push("## 个股评论\n".to_string());
                        sections.push(format_cn_comment_table(arr));
                    } else {
                        sections.push("## 个股评论\n\n> 无数据\n".to_string());
                    }
                } else {
                    sections.push("## 个股评论\n\n> 获取失败\n".to_string());
                }
            } else {
                sections.push("## 个股评论\n\n> 获取失败\n".to_string());
            }
        }
        Err(_) => {
            sections.push("## 个股评论\n\n> 获取失败\n".to_string());
        }
    }

    // ── 机构参与度 ──
    let eval_url = format!(
        "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_DMSK_TS_STOCKEVALUATE&filter=(SECURITY_CODE=%22{}%22)&columns=ALL&source=WEB&client=WEB&sortColumns=TRADE_DATE&sortTypes=-1",
        symbol
    );

    match get_with_retry(client, &eval_url, Some(2)).await {
        Ok(resp) => {
            if let Ok(body) = crate::http::json_limited(resp).await {
                let data_arr = body
                    .get("result")
                    .and_then(|r| r.get("data"))
                    .and_then(|d| d.as_array());

                if let Some(arr) = data_arr {
                    if !arr.is_empty() {
                        has_data = true;
                        sections.push("## 机构参与度\n".to_string());
                        sections.push(format_org_participation_table(arr));
                    } else {
                        sections.push("## 机构参与度\n\n> 无数据\n".to_string());
                    }
                } else {
                    sections.push("## 机构参与度\n\n> 获取失败\n".to_string());
                }
            } else {
                sections.push("## 机构参与度\n\n> 获取失败\n".to_string());
            }
        }
        Err(_) => {
            sections.push("## 机构参与度\n\n> 获取失败\n".to_string());
        }
    }

    if !has_data {
        sections.push("\n> A 股情绪数据源全部不可用\n".to_string());
    }

    sections.join("\n")
}

/// 格式化A股千股千评数据表格
fn format_cn_comment_table(data: &[Value]) -> String {
    // 展示最近 10 条的关键字段：(显示名, 东方财富字段名, 是否整数)
    // 字段名对应 datacenter RPT_DMSK_TS_STOCKNEW 的实际返回字段。
    let indicators = [
        ("交易日期", "TRADE_DATE", false),
        ("收盘价", "CLOSE_PRICE", false),
        ("涨跌幅(%)", "CHANGE_RATE", false),
        ("综合得分", "TOTALSCORE", false),
        ("目前排名", "RANK", true),
        ("关注指数", "FOCUS", false),
    ];

    let display_count = data.len().min(10);
    let mut lines = Vec::new();

    // 表头
    let header: Vec<&str> = indicators.iter().map(|(d, _, _)| *d).collect();
    lines.push(format!("| {} |", header.join(" | ")));
    // 分隔行：N 列需 N+1 个 `|`。`"---|".repeat(N)` 末尾已带 `|`，
    // 外层再补 `|` 会产生尾随 `||`，使分隔行列数多于表头/数据行。
    lines.push(format!("|{}", "---|".repeat(indicators.len())));

    for row in data.iter().take(display_count) {
        let mut cells = Vec::new();
        for (_, field, is_int) in &indicators {
            let val = row.get(*field).unwrap_or(&Value::Null);
            let s = match val {
                Value::Null => "N/A".to_string(),
                Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        if *is_int {
                            format!("{:.0}", f)
                        } else {
                            format!("{:.2}", f)
                        }
                    } else {
                        n.to_string()
                    }
                }
                Value::String(s) => s.clone(),
                _ => val.to_string(),
            };
            cells.push(s);
        }
        lines.push(format!("| {} |", cells.join(" | ")));
    }

    lines.join("\n") + "\n"
}

/// 格式化机构参与度数据表格
fn format_org_participation_table(data: &[Value]) -> String {
    let display_count = data.len().min(10);
    let mut lines = Vec::new();

    lines.push("| 交易日期 | 机构参与度 |".to_string());
    lines.push("|---|---|".to_string());

    for row in data.iter().take(display_count) {
        let date = row
            .get("TRADE_DATE")
            .and_then(|v| v.as_str())
            .map(|s| s.get(..10).unwrap_or(s))
            .unwrap_or("N/A");

        // 机构参与度需乘以100转为百分比。
        // 实测量标（2026-08-11 对 002594 RPT_DMSK_TS_STOCKEVALUATE 线上接口）：
        // ORG_PARTICIPATE 返回 [0,1) 小数（0.39679 / 0.4476208 / 0.4510624 ...），
        // 即 39.68% / 44.76% / 45.11% —— 必须 *100；不要改为"已含百分号"直接输出。
        // 锁定此行为的单测：test_format_org_participation_table。
        let org_pct = row
            .get("ORG_PARTICIPATE")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.2}%", v * 100.0))
            .unwrap_or_else(|| "N/A".to_string());

        lines.push(format!("| {} | {} |", date, org_pct));
    }

    lines.join("\n") + "\n"
}

// ───────────────────────── 统一入口 ─────────────────────────

/// 统一市场情绪数据获取入口
///
/// 自动检测市场类型：
/// - A股（6位纯数字）→ 千股千评 + 机构参与度
/// - 港股（.HK / 5位数字）→ 暂不支持（StockTwits/Reddit 仅覆盖美股，东方财富未覆盖港股）
/// - 其他 → StockTwits + Reddit（并行获取）
///
/// `days` 控制 Reddit 帖子时间窗（≤7 天按 week、>7 天按 month）。
/// 契约：永不 panic，错误以 Err(String) 返回（调用方不再靠字符串前缀探测错误）。
pub async fn fetch_sentiment(
    client: &Client,
    symbol: &str,
    limit: u32,
    days: u32,
) -> Result<String, String> {
    let out = fetch_sentiment_inner(client, symbol, limit, days).await;
    if out.starts_with("错误") {
        Err(out)
    } else {
        Ok(out)
    }
}

async fn fetch_sentiment_inner(client: &Client, symbol: &str, limit: u32, days: u32) -> String {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        return "错误: 股票代码不能为空".to_string();
    }

    let market = detect_market(symbol);
    match market {
        Market::CNStock => fetch_cn_sentiment(client, symbol).await,
        Market::HKStock => {
            // 港股：StockTwits/Reddit 仅覆盖美股，东方财富千股千评/机构参与度未覆盖港股，
            // 不发起无效请求，直接给出明确提示。
            format!(
                "# {} 综合情绪分析报告\n\n## 市场情绪\n\n> 港股情绪数据暂不支持：StockTwits/Reddit 仅覆盖美股，东方财富千股千评/机构参与度未覆盖港股。\n> 若该股有美股对应代码（如 9988.HK -> BABA），可用该美股代码查询情绪。\n",
                symbol
            )
        }
        _ => {
            // 美股/其他：并行获取 StockTwits + Reddit
            let (stocktwits_result, reddit_result) = tokio::join!(
                fetch_stocktwits(client, symbol, limit),
                fetch_reddit_sentiment(client, symbol, days)
            );

            let mut sections = Vec::new();
            sections.push(format!("# {} 综合情绪分析报告\n", symbol));

            if stocktwits_result == "<unavailable>" {
                sections.push("## StockTwits 情绪\n\n> 数据源不可用\n".to_string());
            } else {
                sections.push(stocktwits_result);
            }
            sections.push("\n---\n".to_string());

            if reddit_result == "<unavailable>" {
                sections.push("## Reddit 情绪\n\n> 数据源不可用\n".to_string());
            } else {
                sections.push(reddit_result);
            }

            sections.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 验证 A股千股千评使用正确的东方财富字段名（修复 综合得分/目前排名/关注指数 N/A）。
    #[test]
    fn test_format_cn_comment_table() {
        use serde_json::json;
        let data = vec![json!({
            "TRADE_DATE": "2026-07-31 00:00:00",
            "CLOSE_PRICE": 95.77,
            "CHANGE_RATE": -0.073,
            "TOTALSCORE": 76.84826875,
            "RANK": 104,
            "FOCUS": 94.4
        })];
        let table = format_cn_comment_table(&data);
        assert!(table.contains("76.85"), "TOTALSCORE 综合得分");
        assert!(table.contains("94.40"), "FOCUS 关注指数");
        // RANK 应整数显示（不含小数）
        assert!(table.contains("| 104 |"), "RANK 目前排名应整数: {}", table);
        // 无 N/A
        assert!(!table.contains("N/A"), "不应有 N/A: {}", table);
        // 分隔行（第二行）`|` 数必须等于表头；修复前多一个尾随 `|` 形如 `|---|---|---|---|---|---||`。
        let lines: Vec<&str> = table.lines().collect();
        let pipes = |s: &str| s.chars().filter(|&c| c == '|').count();
        assert!(
            !lines[1].ends_with("||"),
            "分隔行不应以 || 结尾: {}",
            lines[1]
        );
        assert_eq!(pipes(lines[0]), pipes(lines[1]), "分隔行管道符数应等于表头");
    }

    // 锁定 ORG_PARTICIPATE 缩放（实测量标 0.39679 → 39.68%，必须 *100）。
    #[test]
    fn test_format_org_participation_table() {
        use serde_json::json;
        // 实测量标数据（2026-08-11 002594）：ORG_PARTICIPATE 是 [0,1) 小数。
        let data = vec![
            json!({"TRADE_DATE": "2026-08-11 00:00:00", "ORG_PARTICIPATE": 0.39679}),
            json!({"TRADE_DATE": "2026-08-10 00:00:00", "ORG_PARTICIPATE": 0.4476208}),
            json!({"TRADE_DATE": "2026-08-05 00:00:00", "ORG_PARTICIPATE": 0.4109164}),
        ];
        let table = format_org_participation_table(&data);
        assert!(
            table.contains("39.68%"),
            "0.39679 应显示为 39.68%: {}",
            table
        );
        assert!(
            table.contains("44.76%"),
            "0.4476208 应显示为 44.76%: {}",
            table
        );
        assert!(
            table.contains("41.09%"),
            "0.4109164 应显示为 41.09%: {}",
            table
        );
        // 若误删 *100 会输出 0.40%——显式防回归
        assert!(!table.contains("0.40%"), "小数不应原样输出: {}", table);
        // 日期取前 10 字符
        assert!(table.contains("2026-08-11"), "日期应保留: {}", table);
        assert!(!table.contains("N/A"), "不应有 N/A: {}", table);
    }

    // A股情绪走东方财富，国内网络即可达。
    #[tokio::test]
    #[ignore = "hits the live Eastmoney API"]
    async fn test_live_sentiment_cn() {
        let client = crate::http::build_client().unwrap();
        let out = fetch_sentiment(&client, "002594", 15, 7)
            .await
            .unwrap_or_else(|e| panic!("002594 情绪不应报错: {}", e));
        assert!(out.contains("个股评论"), "应包含个股评论段落");
        // 修复后 综合得分/目前排名/关注指数 不应再全为 N/A
        assert!(
            !out.contains("| N/A | N/A | N/A |"),
            "综合得分/排名/关注指数不应全 N/A"
        );
    }
}
