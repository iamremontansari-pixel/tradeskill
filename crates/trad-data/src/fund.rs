//! 东方财富基金数据抓取：净值历史 / 基金资料 / 重仓股 / 业绩表现。
//!
//! 所有基金接口都要求浏览器 UA；NAV/重仓/业绩接口还要求 `Referer` 反爬头。
//! 响应体实际为 GBK 编码（即使响应头标注 charset=utf-8），统一按 GBK 解码。
//! 解析只用标准库字符串方法，不引入 HTML 解析 crate。

use chrono::{Duration, Local};

/// 单日净值行（FSRQ→date, DWJZ→unit_nav, LJJZ→acc_nav, JZZZL→growth_pct,
/// SGZT→sub_status, SHZT→redempt_status）
pub struct NavRow {
    pub date: String,
    pub unit_nav: f64,
    pub acc_nav: f64,
    pub growth_pct: f64,
    pub sub_status: String,
    pub redempt_status: String,
}

/// 基金基本资料（来源：基金档案页 jbgk HTML 表格）
pub struct FundProfile {
    pub full_name: String,
    pub fund_type: String,
    pub inception_date: String,
    pub aum: String,
    pub manager: String,
    pub management_co: String,
    pub custodian: String,
}

/// 重仓股行（占净值比例 / 持股数(万股) / 持仓市值(万元)）
pub struct HoldingRow {
    pub rank: u32,
    pub stock_code: String,
    pub stock_name: String,
    pub weight_pct: f64,
    pub shares_wan: f64,
    pub value_wan: f64,
}

/// 业绩表现行（数值原样取自页面文本，保留 % 与排名格式）
pub struct PerfRow {
    pub period: String,
    pub return_pct: String,
    pub peer_avg_pct: String,
    pub benchmark_pct: String,
    pub peer_rank: String,
}

/// 业绩表
pub type PerfTable = Vec<PerfRow>;

/// 带浏览器 UA + Referer 的基金接口 GET，响应按需解码。
///
/// 2026-08-08 起东方财富基金接口实际已全部返回 UTF-8（响应头 charset=utf-8
/// 属实）；此前曾返回 GBK 字节。优先严格 UTF-8 解码，失败再回退 GBK，
/// 以兼容两种状态的边缘节点。大小上限沿用 http.rs 的 50 MB 流式防护。
async fn fetch_fund_body(
    client: &reqwest::Client,
    url: &str,
    referer: &str,
) -> Result<String, String> {
    let resp = crate::http::get_with_retry_headers(
        client,
        url,
        &[("User-Agent", "Mozilla/5.0"), ("Referer", referer)],
        Some(3),
    )
    .await
    .map_err(|e| format!("东方财富 API 请求失败: {}", e))?;
    let bytes = crate::http::bytes_limited(resp).await?;
    match std::str::from_utf8(&bytes) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            let (text, _, _) = encoding_rs::GBK.decode(&bytes);
            Ok(text.into_owned())
        }
    }
}

/// 拉取基金净值历史（[start = 今天 - days, end = 今天]，最新在前）。
///
/// 数据缺失（ErrCode != 0 / Data 为 null / 无 LSJZList）返回空 Vec 而非 Err。
pub async fn fetch_fund_nav(
    client: &reqwest::Client,
    code: &str,
    days: u32,
) -> Result<Vec<NavRow>, String> {
    let end = Local::now().format("%Y-%m-%d").to_string();
    let start = (Local::now() - Duration::days(days as i64))
        .format("%Y-%m-%d")
        .to_string();
    // lsjz 接口 pageSize 上限 200：超限（如默认 days=365 时的 395）直接返回
    // Data:null。返回最新在前，超出上限的行仅影响更早日期，不影响展示的尾部。
    let page_size = days.saturating_add(30).min(200);
    let url = format!(
        "https://api.fund.eastmoney.com/f10/lsjz?callback=&fundCode={}&pageIndex=1&pageSize={}&startDate={}&endDate={}",
        code, page_size, start, end
    );
    let body = fetch_fund_body(client, &url, "https://fund.eastmoney.com/").await?;
    Ok(parse_nav_json(&body))
}

/// 拉取基金基本资料（基金全称/类型/成立日期/规模/经理/管理人/托管人）。
pub async fn fetch_fund_profile(
    client: &reqwest::Client,
    code: &str,
) -> Result<FundProfile, String> {
    let url = format!("https://fundf10.eastmoney.com/jbgk_{}.html", code);
    let body = fetch_fund_body(client, &url, &url).await?;
    parse_profile_html(&body)
}

/// 拉取前十大重仓股（year 参数留空 = 最新一期季报）。
pub async fn fetch_fund_holdings(
    client: &reqwest::Client,
    code: &str,
) -> Result<Vec<HoldingRow>, String> {
    let url = format!(
        "https://fundf10.eastmoney.com/FundArchivesDatas.aspx?type=jjcc&code={}&topline=10&year=",
        code
    );
    let referer = format!("https://fundf10.eastmoney.com/ccmx_{}.html", code);
    let body = fetch_fund_body(client, &url, &referer).await?;
    Ok(parse_holdings_js(&body))
}

/// 拉取业绩表现（近1月/3月/6月/1年/3年/成立以来等区间涨幅与同类排名）。
pub async fn fetch_fund_performance(
    client: &reqwest::Client,
    code: &str,
) -> Result<PerfTable, String> {
    let url = format!(
        "https://fundf10.eastmoney.com/FundArchivesDatas.aspx?type=jdzf&code={}",
        code
    );
    let referer = format!("https://fundf10.eastmoney.com/jdzf_{}.html", code);
    let body = fetch_fund_body(client, &url, &referer).await?;
    Ok(parse_performance_js(&body))
}

// ───────────────────────── 报告调度与格式化 ─────────────────────────

/// 统一基金数据获取入口：并行拉取净值/资料/重仓/业绩，拼装紧凑 markdown 报告。
///
/// 任一数据源失败时该小节降级为 `> 该数据源暂不可用` 提示，其余小节正常输出；
/// 仅当 4 个数据源均无可用数据（失败或空结果）时返回 `Err`（exit 1）。`tail`
/// 只截断净值表的展示行数（NAV 接口已按 `days` 窗口取数，不重新请求）。
pub async fn fetch_fund(
    client: &reqwest::Client,
    code: &str,
    tail: u32,
    days: u32,
) -> Result<String, String> {
    let (nav, profile, holdings, perf) = tokio::join!(
        fetch_fund_nav(client, code, days),
        fetch_fund_profile(client, code),
        fetch_fund_holdings(client, code),
        fetch_fund_performance(client, code),
    );
    assemble_fund_report(code, tail, days, nav, profile, holdings, perf)
}

/// 将 4 个数据源的抓取结果组装为最终报告（与网络解耦，便于 mock 测试）。
///
/// profile 失败时基金名回退为基金代码；4 个数据源均无可用数据（Err 或 Ok
/// 空结果——无效基金代码的典型响应）时返回 `Err`（带“错误: ”前缀，与其它
/// 子命令的 exit 1 契约一致）。
fn assemble_fund_report(
    code: &str,
    tail: u32,
    days: u32,
    nav: Result<Vec<NavRow>, String>,
    profile: Result<FundProfile, String>,
    holdings: Result<Vec<HoldingRow>, String>,
    perf: Result<PerfTable, String>,
) -> Result<String, String> {
    let fund_name = match &profile {
        Ok(p) if !p.full_name.is_empty() => p.full_name.clone(),
        _ => code.to_string(),
    };
    let report = build_fund_report(
        code,
        &fund_name,
        nav.as_deref().ok(),
        profile.as_ref().ok(),
        holdings.as_deref().ok(),
        perf.as_deref().ok(),
        tail,
        days,
    );
    let nav_usable = nav.as_ref().is_ok_and(|v| !v.is_empty());
    let profile_usable = profile.is_ok();
    let holdings_usable = holdings.as_ref().is_ok_and(|v| !v.is_empty());
    let perf_usable = perf.as_ref().is_ok_and(|v| !v.is_empty());
    if !nav_usable && !profile_usable && !holdings_usable && !perf_usable {
        return Err(format!(
            "错误: 基金 {} 数据获取失败（净值/资料/重仓/业绩均无可用数据）",
            code
        ));
    }
    Ok(report)
}

/// 构建基金紧凑报告（各数据源已抓取的数据；`None` = 该数据源不可用）。
///
/// 每个不可用的小节渲染 `> 该数据源暂不可用` 提示；NAV 表按 `tail` 截取最新
/// N 行（接口返回已是最新在前）。只引用关键数字，不复刻原始 API 响应。
#[allow(clippy::too_many_arguments)]
fn build_fund_report(
    code: &str,
    fund_name: &str,
    nav_rows: Option<&[NavRow]>,
    profile: Option<&FundProfile>,
    holdings: Option<&[HoldingRow]>,
    perf_rows: Option<&[PerfRow]>,
    tail: u32,
    days: u32,
) -> String {
    let mut sections = Vec::new();
    sections.push(format!("# {} {} 基金分析报告\n", code, fund_name));

    sections.push("## 基金概况\n".to_string());
    match profile {
        Some(p) => sections.push(build_profile_table(p)),
        None => sections.push("> 该数据源暂不可用\n".to_string()),
    }

    // 标题同时展示取数窗口（days）与展示行数（tail），避免把显示行数误当取数窗
    sections.push(format!(
        "## 净值历史 (近 {} 日, 显示最新 {} 条)\n",
        days, tail
    ));
    match nav_rows {
        Some(rows) if !rows.is_empty() => {
            let tail_n = tail as usize;
            let shown = if rows.len() > tail_n {
                &rows[..tail_n]
            } else {
                rows
            };
            sections.push(build_nav_table(shown));
            // L20：lsjz 接口 pageSize 上限 200，days 大时请求可能被截断
            sections.push(format!(
                "> 注：东方财富 lsjz 接口 pageSize 上限 200，近 {} 日请求可能只返回最新 200 条 NAV（约 1 年交易日）。\n",
                days
            ));
        }
        _ => sections.push("> 该数据源暂不可用\n".to_string()),
    }

    sections.push("## 重仓股 (Top 10)\n".to_string());
    match holdings {
        Some(rows) if !rows.is_empty() => sections.push(build_holdings_table(rows)),
        _ => sections.push("> 该数据源暂不可用\n".to_string()),
    }

    sections.push("## 业绩表现\n".to_string());
    match perf_rows {
        Some(rows) if !rows.is_empty() => sections.push(build_perf_table(rows)),
        _ => sections.push("> 该数据源暂不可用\n".to_string()),
    }

    sections.join("\n")
}

/// 基金概况键值表（空字段显示 N/A，与 fundamentals.rs 约定一致）。
fn build_profile_table(p: &FundProfile) -> String {
    fn cell(v: &str) -> &str {
        if v.is_empty() {
            "N/A"
        } else {
            v
        }
    }
    let rows = [
        ("基金全称", cell(&p.full_name)),
        ("类型", cell(&p.fund_type)),
        ("成立日期", cell(&p.inception_date)),
        ("规模", cell(&p.aum)),
        ("基金经理", cell(&p.manager)),
        ("管理人", cell(&p.management_co)),
        ("托管人", cell(&p.custodian)),
    ];
    let mut lines = Vec::new();
    lines.push("| 项目 | 内容 |".to_string());
    lines.push("|---|---|".to_string());
    for (k, v) in rows {
        lines.push(format!("| {} | {} |", k, v));
    }
    lines.join("\n") + "\n"
}

/// 净值历史表（输入已是最新在前，直接展示前 `tail` 行）。
fn build_nav_table(rows: &[NavRow]) -> String {
    let mut lines = Vec::new();
    lines.push("| 日期 | 单位净值 | 累计净值 | 日涨跌% | 申购状态 | 赎回状态 |".to_string());
    lines.push("|---|---|---|---|---|---|".to_string());
    for r in rows {
        lines.push(format!(
            "| {} | {:.4} | {:.4} | {:.2} | {} | {} |",
            r.date, r.unit_nav, r.acc_nav, r.growth_pct, r.sub_status, r.redempt_status
        ));
    }
    lines.join("\n") + "\n"
}

/// 重仓股表（占净值比例 + 持股数 + 持仓市值，市值按 万/亿 换算）。
fn build_holdings_table(rows: &[HoldingRow]) -> String {
    let mut lines = Vec::new();
    lines.push("| 排名 | 代码 | 名称 | 占净值% | 持股(万股) | 市值 |".to_string());
    lines.push("|---|---|---|---|---|---|".to_string());
    for r in rows {
        lines.push(format!(
            "| {} | {} | {} | {:.2}% | {:.2} | {} |",
            r.rank,
            r.stock_code,
            r.stock_name,
            r.weight_pct,
            r.shares_wan,
            fmt_wan_amount(r.value_wan)
        ));
    }
    lines.join("\n") + "\n"
}

/// 万元金额格式化：≥1亿元显示「X.XX亿」，否则显示「X万」。
fn fmt_wan_amount(v: f64) -> String {
    if v >= 10_000.0 {
        format!("{:.2}亿", v / 10_000.0)
    } else {
        format!("{:.0}万", v)
    }
}

/// 业绩表现表（区间涨幅/同类平均/沪深300/同类排名，数值原样取自页面）。
fn build_perf_table(rows: &[PerfRow]) -> String {
    let mut lines = Vec::new();
    lines.push("| 区间 | 涨幅 | 同类平均 | 沪深300 | 同类排名 |".to_string());
    lines.push("|---|---|---|---|---|".to_string());
    for r in rows {
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            r.period, r.return_pct, r.peer_avg_pct, r.benchmark_pct, r.peer_rank
        ));
    }
    lines.join("\n") + "\n"
}

/// 解析净值 JSON；ErrCode != 0 或 Data/LSJZList 缺失 → 空 Vec（视为无数据）。
fn parse_nav_json(body: &str) -> Vec<NavRow> {
    let root: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if root.get("ErrCode").and_then(|c| c.as_i64()) != Some(0) {
        return Vec::new();
    }
    let list = match root
        .get("Data")
        .and_then(|d| d.get("LSJZList"))
        .and_then(|l| l.as_array())
    {
        Some(list) => list,
        None => return Vec::new(),
    };
    let mut rows = Vec::new();
    for item in list {
        let date = match item.get("FSRQ").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        // 单位净值是核心字段，解析失败则跳过整行
        let unit_nav: f64 = match item
            .get("DWJZ")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
        {
            Some(v) => v,
            None => continue,
        };
        // 累计净值/涨跌幅缺失或为 ""（首行常见）按 0 处理，不丢行
        let acc_nav = item
            .get("LJJZ")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let growth_pct = item
            .get("JZZZL")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        rows.push(NavRow {
            date,
            unit_nav,
            acc_nav,
            growth_pct,
            sub_status: item
                .get("SGZT")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            redempt_status: item
                .get("SHZT")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    rows
}

/// 解析档案页 HTML 表格为 FundProfile。
///
/// 关键字段全部缺失（404/反爬页）视为解析失败，返回 Err。
fn parse_profile_html(html: &str) -> Result<FundProfile, String> {
    let field = |label: &str| extract_html_table_field(html, label).unwrap_or_default();
    let profile = FundProfile {
        full_name: field("基金全称"),
        fund_type: field("基金类型"),
        inception_date: field("成立日期"),
        aum: field("资产规模"),
        manager: field("基金经理"),
        management_co: field("管理人"),
        custodian: field("托管人"),
    };
    if profile.full_name.is_empty() && profile.fund_type.is_empty() && profile.manager.is_empty() {
        return Err("基金资料解析失败（页面结构异常）".to_string());
    }
    Ok(profile)
}

/// 从档案页 HTML 表格中按表头标签提取单元格文本。
///
/// 逐个 `<th>` 检查其文本是否包含 `label`（兼容真实标签带前后缀，如
/// 「基金管理人」包含「管理人」），取该表头后第一个 `<td>` 单元格。
/// 部分单元格缺失 `</td>`（如「净资产规模」），以下一个 `<th>` 为截断边界。
/// 占位值（`---` / `>---`，无效基金代码的占位页形态）视为无数据，跳过。
fn extract_html_table_field(html: &str, label: &str) -> Option<String> {
    let mut pos = 0;
    while let Some(th_start) = html[pos..].find("<th") {
        let th_abs = pos + th_start;
        let th_open_end = html[th_abs..].find('>')? + th_abs + 1;
        let th_close_rel = html[th_open_end..].find("</th>")?;
        let th_text = strip_tags(&html[th_open_end..th_open_end + th_close_rel]);
        if th_text.contains(label) {
            let td_abs =
                html[th_open_end + th_close_rel..].find("<td")? + th_open_end + th_close_rel;
            let td_open_end = html[td_abs..].find('>')? + td_abs + 1;
            let tail = &html[td_open_end..];
            let cell_end = tail
                .find("</td>")
                .unwrap_or(usize::MAX)
                .min(tail.find("<th").unwrap_or(usize::MAX));
            let cell = strip_tags(&html[td_open_end..td_open_end + cell_end]);
            let cell = cell.trim().trim_start_matches('>').trim();
            if !cell.is_empty() && cell != "---" {
                return Some(cell.to_string());
            }
        }
        pos = th_open_end + th_close_rel + "</th>".len();
    }
    None
}

/// 提取 JS 变量 `apidata` 中 `content:"..."` 的字符串值（处理 `\"` 转义）。
fn extract_apidata_content(body: &str) -> Option<String> {
    const MARKER: &str = "content:\"";
    let start = body.find(MARKER)? + MARKER.len();
    let mut out = String::new();
    let mut escaped = false;
    for ch in body[start..].chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

/// 解析重仓股 JS：取 `content` 后按 `<tr>` 行解析。
fn parse_holdings_js(body: &str) -> Vec<HoldingRow> {
    match extract_apidata_content(body) {
        Some(content) => parse_holdings_table(&content),
        None => Vec::new(),
    }
}

/// 解析重仓股表格。列数随页面改版变化（历史 6 列，当前 9 列），
/// 排名/代码/名称取前三列，占净值比例/持股数/市值取末三列，避免列漂移。
fn parse_holdings_table(html: &str) -> Vec<HoldingRow> {
    let mut rows = Vec::new();
    for block in extract_blocks(html, "<tr") {
        let cells: Vec<String> = extract_blocks(block, "<td")
            .iter()
            .map(|c| strip_tags(c).trim().to_string())
            .collect();
        if cells.len() < 6 {
            continue;
        }
        let rank = match cells[0].parse::<u32>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let stock_code = cells[1].clone();
        if stock_code.is_empty() {
            continue;
        }
        let n = cells.len();
        rows.push(HoldingRow {
            rank,
            stock_code,
            stock_name: cells[2].clone(),
            weight_pct: parse_pct(&cells[n - 3]),
            shares_wan: parse_num(&cells[n - 2]),
            value_wan: parse_num(&cells[n - 1]),
        });
    }
    rows
}

/// 解析业绩 JS：取 `content` 后按 `<ul>` 块解析。
fn parse_performance_js(body: &str) -> PerfTable {
    match extract_apidata_content(body) {
        Some(content) => parse_performance_html(&content),
        None => Vec::new(),
    }
}

/// 解析业绩表：每个 `<ul>` 对应一个区间，首个 `<li>` 为区间名
/// （表头行的区间名为空，跳过），其后依次为区间涨幅/同类平均/沪深300/同类排名。
fn parse_performance_html(html: &str) -> PerfTable {
    let mut rows = Vec::new();
    for block in extract_blocks(html, "<ul") {
        let lis: Vec<String> = extract_blocks(block, "<li")
            .iter()
            .map(|s| strip_tags(s).trim().to_string())
            .collect();
        let period = match lis.first().map(String::as_str) {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => continue,
        };
        let mut vals = lis.into_iter().skip(1);
        rows.push(PerfRow {
            period,
            return_pct: vals.next().unwrap_or_default(),
            peer_avg_pct: vals.next().unwrap_or_default(),
            benchmark_pct: vals.next().unwrap_or_default(),
            peer_rank: vals.next().unwrap_or_default(),
        });
    }
    rows
}

/// 提取全部 `<{open}>...</{open}>` 块的内层内容（不含标签本身）。
///
/// open 形如 `"<tr"`；未闭合的最后一个块直接丢弃。内层不会嵌套同类标签
/// （重仓/业绩表格的数据行内没有嵌套 tr/td/li）。
fn extract_blocks<'a>(html: &'a str, open: &str) -> Vec<&'a str> {
    let close = format!("</{}>", &open[1..]);
    let mut blocks = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(open) {
        let open_end = match rest[start..].find('>') {
            Some(i) => start + i + 1,
            None => break,
        };
        match rest[open_end..].find(&close) {
            Some(i) => {
                blocks.push(&rest[open_end..open_end + i]);
                rest = &rest[open_end + i + close.len()..];
            }
            None => break,
        }
    }
    blocks
}

/// 移除字符串中的 HTML 标签（如 `<a href="...">文本</a>` → `文本`），
/// 并解码常见 HTML 实体（`&amp;` → `&`，复用 news.rs 的解码器；顺序上
/// 先剥标签再解码，与 news.rs strip_html 一致）。
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    crate::news::decode_html_entities(&out)
}

/// 解析百分比文本（"9.23%" → 9.23，"--" → 0.0）
fn parse_pct(s: &str) -> f64 {
    parse_num(s.trim_end_matches('%'))
}

/// 解析去千分位的数字文本（"25,400.00" → 25400.0，"--" → 0.0）
fn parse_num(s: &str) -> f64 {
    s.replace(',', "").trim().parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 计划内 fixture（6 列重仓表格 / 简化业绩片段）──

    const NAV_JSON: &str = r#"{"ErrCode":0,"Data":{"LSJZList":[{"FSRQ":"2026-08-07","DWJZ":"4.1945","LJJZ":"5.9845","JZZZL":"0.47","SGZT":"限制大额申购","SHZT":"开放赎回"}]}}"#;

    const NAV_JSON_MISSING: &str = r#"{"ErrCode":1,"Data":null}"#;

    const PROFILE_HTML: &str = r#"<table>
<tr><th>基金全称</th><td>华夏成长证券投资基金</td></tr>
<tr><th>基金类型</th><td>混合型</td></tr>
<tr><th>成立日期/规模</th><td>2001年12月18日 / 32.368亿份</td></tr>
<tr><th>净资产规模</th><td>39.38亿元（截止日期2026年06月30日）</td></tr>
<tr><th>基金管理人</th><td><a href="//fund.eastmoney.com/company/80000222.html">华夏基金</a></td></tr>
<tr><th>基金托管人</th><td><a href="//fund.eastmoney.com/bank/80001068.html">建设银行</a></td></tr>
<tr><th>基金经理</th><td><a href="//fund.eastmoney.com/manager/30040527.html">郑晓蔚</a>、<a href="//fund.eastmoney.com/manager/30786034.html">刘睿思</a></td></tr>
</table>"#;

    const HOLDINGS_JS: &str = r#"var apidata={ content:"<table><tr><td>1</td><td>600519</td><td>贵州茅台</td><td>9.23%</td><td>52.77</td><td>62558.31</td></tr></table>" };"#;

    const PERFORMANCE_JS: &str = r#"var apidata={ content:"<div class='jdzfnew'><ul class='fcol'><li class='title'></li><li>涨幅</li><li>同类平均</li><li>沪深300</li><li>同类排名</li></ul><ul><li class='title'>今年来</li><li class='tor red bold'>23.92%</li><li class='tor red bold'>7.86%</li><li class='tor red bold'>1.39%</li><li class='tlpm'>330<font class='gray'>|</font>2314</li></ul><ul><li class='title'>近1月</li><li class='tor grn bold'>-0.93%</li><li>0.32%</li><li>1.05%</li><li>1250|2345</li></ul></div>" };"#;

    // ── 真实响应形态 fixture（9 列 + 链接 + 千分位）──

    const HOLDINGS_JS_REAL: &str = r#"var apidata={ content:"<table><tbody><tr><td>1</td><td><a href='//quote.eastmoney.com/unify/r/0.300308'>300308</a></td><td class='tol'><a href='//quote.eastmoney.com/unify/r/0.300308'>中际旭创</a></td><td class='tor'><span data-id='dq300308'></span></td><td class='tor'><span data-id='zd300308'></span></td><td class='xglj'><a href='ccbdxq_000001_300308.html' class='red'>变动详情</a></td><td class='tor'>6.45%</td><td class='tor'>20.00</td><td class='tor'>25,400.00</td></tr></tbody></table>" };"#;

    const JS_ESCAPED_QUOTES: &str = r#"var apidata={ content:"<td class=\"x\">贵州茅台</td>" };"#;

    #[test]
    fn test_parse_nav_json() {
        let rows = parse_nav_json(NAV_JSON);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.date, "2026-08-07");
        assert_eq!(row.unit_nav, 4.1945);
        assert_eq!(row.acc_nav, 5.9845);
        assert_eq!(row.growth_pct, 0.47);
        assert_eq!(row.sub_status, "限制大额申购");
        assert_eq!(row.redempt_status, "开放赎回");
    }

    #[test]
    fn test_parse_nav_missing_data() {
        // ErrCode != 0 → 空 Vec，不 panic、不报错
        assert!(parse_nav_json(NAV_JSON_MISSING).is_empty());
    }

    #[test]
    fn test_parse_nav_malformed() {
        // 非 JSON / 缺字段 / 数值非法 → 空 Vec，不 panic
        assert!(parse_nav_json("not json at all").is_empty());
        assert!(
            parse_nav_json(r#"{"ErrCode":0,"Data":{"LSJZList":[{"FSRQ":"2026-08-07"}]}}"#)
                .is_empty()
        );
        assert!(
            parse_nav_json(r#"{"ErrCode":0,"Data":{"LSJZList":[{"FSRQ":"","DWJZ":"1.0"}]}}"#)
                .is_empty()
        );
    }

    #[test]
    fn test_parse_profile_html() {
        let profile = parse_profile_html(PROFILE_HTML).expect("应解析成功");
        assert_eq!(profile.full_name, "华夏成长证券投资基金");
        assert_eq!(profile.fund_type, "混合型");
        // 真实标签带前后缀：「成立日期/规模」「净资产规模」都能命中
        assert_eq!(profile.inception_date, "2001年12月18日 / 32.368亿份");
        // 「净资产规模」单元格缺失 </td>，应以下一个 <th> 截断
        assert_eq!(profile.aum, "39.38亿元（截止日期2026年06月30日）");
        assert_eq!(profile.manager, "郑晓蔚、刘睿思");
        assert_eq!(profile.management_co, "华夏基金");
        assert_eq!(profile.custodian, "建设银行");
    }

    #[test]
    fn test_parse_profile_missing() {
        // 页面取不到任何关键字段 → Err（404/反爬页）
        assert!(parse_profile_html("<html><body>404 Not Found</body></html>").is_err());
        assert!(parse_profile_html("").is_err());
    }

    #[test]
    fn test_parse_profile_placeholder() {
        // 无效基金代码的占位页：表格齐全但所有值都是 `---`/`>---`，
        // 应视为无数据 → Err（CLI 侧 exit 1），而不是输出空壳报告。
        let placeholder = r#"<table class="info w790"><tr><th>基金全称</th><td>---</td><th>基金简称</th><td>---</td></tr><tr><th>基金代码</th><td>&gt;---</td><th>基金类型</th><td>---</td></tr><tr><th>基金经理人</th><td>---</td><th>基金托管人</th><td>---</td></tr></table>"#;
        assert!(parse_profile_html(placeholder).is_err());
    }

    #[test]
    fn test_parse_holdings_js() {
        let rows = parse_holdings_js(HOLDINGS_JS);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.rank, 1);
        assert_eq!(row.stock_code, "600519");
        assert_eq!(row.stock_name, "贵州茅台");
        assert_eq!(row.weight_pct, 9.23);
        assert_eq!(row.shares_wan, 52.77);
        assert_eq!(row.value_wan, 62558.31);
    }

    #[test]
    fn test_parse_holdings_real_shape() {
        // 真实页面：9 列、代码/名称带 <a> 链接、市值带千分位逗号
        let rows = parse_holdings_js(HOLDINGS_JS_REAL);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.rank, 1);
        assert_eq!(row.stock_code, "300308");
        assert_eq!(row.stock_name, "中际旭创");
        assert_eq!(row.weight_pct, 6.45);
        assert_eq!(row.shares_wan, 20.0);
        assert_eq!(row.value_wan, 25400.0);
    }

    #[test]
    fn test_parse_holdings_malformed() {
        // 无 content / 无表格 / 行缺列 → 空 Vec，不 panic
        assert!(parse_holdings_js("var apidata={ other:1 };").is_empty());
        assert!(parse_holdings_js("garbage").is_empty());
        assert!(parse_holdings_js(
            r#"var apidata={ content:"<table><tr><td>1</td></tr></table>" };"#
        )
        .is_empty());
    }

    #[test]
    fn test_extract_apidata_escaped_quotes() {
        // content 内的 \" 是转义引号，不应提前终止字符串
        let content = extract_apidata_content(JS_ESCAPED_QUOTES).expect("应提取成功");
        assert_eq!(content, r#"<td class="x">贵州茅台</td>"#);
    }

    #[test]
    fn test_parse_performance_js() {
        let rows = parse_performance_js(PERFORMANCE_JS);
        assert_eq!(rows.len(), 2, "表头 ul（区间名为空）应被跳过");
        let first = &rows[0];
        assert_eq!(first.period, "今年来");
        assert_eq!(first.return_pct, "23.92%");
        assert_eq!(first.peer_avg_pct, "7.86%");
        assert_eq!(first.benchmark_pct, "1.39%");
        // 排名 li 内含嵌套 <font> 标签，应被剥除
        assert_eq!(first.peer_rank, "330|2314");
        let second = &rows[1];
        assert_eq!(second.period, "近1月");
        assert_eq!(second.return_pct, "-0.93%");
        assert_eq!(second.peer_rank, "1250|2345");
    }

    #[test]
    fn test_parse_performance_malformed() {
        assert!(parse_performance_js("var apidata={ other:1 };").is_empty());
        assert!(parse_performance_js("garbage").is_empty());
        // 有 ul 但首个 li 为空（表头形态）→ 空 Vec
        assert!(parse_performance_js(
            r#"var apidata={ content:"<ul><li class='title'></li><li>涨幅</li></ul>" };"#
        )
        .is_empty());
    }

    // ── 报告格式化（mock 抓取结果，不走网络）──

    fn mock_nav_rows() -> Vec<NavRow> {
        vec![
            NavRow {
                date: "2026-08-07".to_string(),
                unit_nav: 4.1945,
                acc_nav: 5.9845,
                growth_pct: 0.47,
                sub_status: "限制大额申购".to_string(),
                redempt_status: "开放赎回".to_string(),
            },
            NavRow {
                date: "2026-08-06".to_string(),
                unit_nav: 4.1748,
                acc_nav: 5.9648,
                growth_pct: -0.31,
                sub_status: "限制大额申购".to_string(),
                redempt_status: "开放赎回".to_string(),
            },
            NavRow {
                date: "2026-08-05".to_string(),
                unit_nav: 4.1878,
                acc_nav: 5.9778,
                growth_pct: 1.02,
                sub_status: "开放申购".to_string(),
                redempt_status: "开放赎回".to_string(),
            },
        ]
    }

    fn mock_profile() -> FundProfile {
        FundProfile {
            full_name: "华夏成长证券投资基金".to_string(),
            fund_type: "混合型".to_string(),
            inception_date: "2001年12月18日".to_string(),
            aum: "39.38亿元".to_string(),
            manager: "郑晓蔚、刘睿思".to_string(),
            management_co: "华夏基金".to_string(),
            custodian: "建设银行".to_string(),
        }
    }

    fn mock_holdings() -> Vec<HoldingRow> {
        vec![
            HoldingRow {
                rank: 1,
                stock_code: "600519".to_string(),
                stock_name: "贵州茅台".to_string(),
                weight_pct: 9.23,
                shares_wan: 52.77,
                value_wan: 62558.31,
            },
            HoldingRow {
                rank: 2,
                stock_code: "300308".to_string(),
                stock_name: "中际旭创".to_string(),
                weight_pct: 6.45,
                shares_wan: 20.0,
                value_wan: 25400.0,
            },
        ]
    }

    fn mock_perf() -> Vec<PerfRow> {
        vec![
            PerfRow {
                period: "今年来".to_string(),
                return_pct: "23.92%".to_string(),
                peer_avg_pct: "7.86%".to_string(),
                benchmark_pct: "1.39%".to_string(),
                peer_rank: "330|2314".to_string(),
            },
            PerfRow {
                period: "近1月".to_string(),
                return_pct: "-0.93%".to_string(),
                peer_avg_pct: "0.32%".to_string(),
                benchmark_pct: "1.05%".to_string(),
                peer_rank: "1250|2345".to_string(),
            },
        ]
    }

    #[test]
    fn test_strip_tags_decodes_entities() {
        // HTML 实体应被解码（L18）：公司名 `A &amp; B` → `A & B`
        assert_eq!(strip_tags("A &amp; B"), "A & B");
        assert_eq!(
            strip_tags("<a href='//fund.eastmoney.com/company/1.html'>华夏基金 &amp; 子公司</a>"),
            "华夏基金 & 子公司"
        );
        // 数字实体同样解码
        assert_eq!(strip_tags("it&#39;s &amp; that"), "it's & that");
        // 字面 &（非实体）原样保留
        assert_eq!(strip_tags("Tom & Jerry"), "Tom & Jerry");
    }

    #[test]
    fn test_report_format() {
        // 3 行 mock 净值，tail=2：验证标题 + 4 小节 + 每节一张表 + 尾部截断
        let report = build_fund_report(
            "000001",
            "华夏成长证券投资基金",
            Some(&mock_nav_rows()),
            Some(&mock_profile()),
            Some(&mock_holdings()),
            Some(&mock_perf()),
            2,
            365,
        );
        assert!(report.contains("# 000001 华夏成长证券投资基金 基金分析报告"));
        for section in [
            "## 基金概况",
            "## 净值历史 (近 365 日, 显示最新 2 条)",
            "## 重仓股 (Top 10)",
            "## 业绩表现",
        ] {
            assert!(report.contains(section), "缺少小节 {}", section);
        }
        // L20：pageSize 上限说明应出现在有 NAV 数据的报告中
        assert!(report.contains("pageSize 上限 200"));
        assert!(report.contains("近 365 日请求"));
        // 每节至少一张表
        assert!(report.contains("| 项目 | 内容 |"));
        assert!(report.contains("| 日期 | 单位净值 |"));
        assert!(report.contains("| 排名 | 代码 |"));
        assert!(report.contains("| 区间 | 涨幅 |"));
        // tail 截断：只显示最新 2 行
        assert!(report.contains("2026-08-07"));
        assert!(report.contains("2026-08-06"));
        assert!(!report.contains("2026-08-05"));
        // 引用关键数字而非原始响应
        assert!(report.contains("4.1945"));
        assert!(report.contains("贵州茅台"));
        assert!(report.contains("9.23%"));
        assert!(report.contains("6.26亿"));
        assert!(report.contains("23.92%"));
    }

    #[test]
    fn test_report_graceful_degradation() {
        // NAV 数据源失败 → 该小节降级为提示，其余小节正常输出
        let report = build_fund_report(
            "000001",
            "华夏成长证券投资基金",
            None,
            Some(&mock_profile()),
            Some(&mock_holdings()),
            Some(&mock_perf()),
            30,
            365,
        );
        assert!(report.contains("## 净值历史 (近 365 日, 显示最新 30 条)"));
        // NAV 小节不可用时无 pageSize 说明（说明只随数据表出现）
        assert!(!report.contains("pageSize 上限 200"));
        assert_eq!(report.matches("> 该数据源暂不可用").count(), 1);
        assert!(!report.contains("| 日期 | 单位净值 |"));
        assert!(report.contains("| 项目 | 内容 |"));
        assert!(report.contains("| 排名 | 代码 |"));
        assert!(report.contains("| 区间 | 涨幅 |"));
    }

    #[test]
    fn test_report_all_fail() {
        let result = assemble_fund_report(
            "000001",
            30,
            365,
            Err("净值失败".to_string()),
            Err("资料失败".to_string()),
            Err("重仓失败".to_string()),
            Err("业绩失败".to_string()),
        );
        let err = result.expect_err("全部数据源失败应返回 Err");
        assert!(err.contains("000001"));
    }

    #[test]
    fn test_report_no_data() {
        // 无效基金代码的典型响应：NAV/重仓/业绩 Ok 但为空，profile Err。
        // 无任何可用数据 → 应返回 Err（CLI 侧 exit 1），而不是空壳报告 exit 0。
        let result = assemble_fund_report(
            "999999",
            30,
            365,
            Ok(Vec::new()),
            Err("资料失败".to_string()),
            Ok(Vec::new()),
            Ok(Vec::new()),
        );
        let err = result.expect_err("无任何可用数据应返回 Err");
        assert!(err.contains("999999"));
    }

    #[test]
    fn test_report_fund_name_fallback() {
        // profile 失败 → 标题基金名回退为代码，概况小节降级
        let result = assemble_fund_report(
            "000001",
            30,
            365,
            Ok(mock_nav_rows()),
            Err("资料失败".to_string()),
            Ok(mock_holdings()),
            Ok(mock_perf()),
        );
        let report = result.expect("部分数据源失败仍应返回 Ok");
        assert!(report.contains("# 000001 000001 基金分析报告"));
        assert!(report.contains("> 该数据源暂不可用"));
        assert!(report.contains("| 日期 | 单位净值 |"));
    }

    // ── 实网集成测试（默认忽略；手动运行：cargo test -- --ignored）──

    #[tokio::test]
    #[ignore = "hits the live Eastmoney fund API"]
    async fn test_live_fund_000001() {
        let client = crate::http::build_client().unwrap();
        let report = fetch_fund(&client, "000001", 30, 365)
            .await
            .expect("000001 华夏成长 基金报告应返回数据");
        assert!(!report.is_empty(), "000001 基金报告不应为空");
        assert!(
            !report.starts_with("错误"),
            "000001 基金报告不应以错误开头: {}",
            report
        );
        assert!(report.contains("# 000001"), "报告应包含 # 000001 标题");
    }

    #[tokio::test]
    #[ignore = "hits the live Eastmoney fund API"]
    async fn test_live_fund_510300() {
        let client = crate::http::build_client().unwrap();
        let report = fetch_fund(&client, "510300", 30, 365)
            .await
            .expect("510300 沪深300ETF 基金报告应返回数据");
        assert!(!report.is_empty(), "510300 基金报告不应为空");
        assert!(
            !report.starts_with("错误"),
            "510300 基金报告不应以错误开头: {}",
            report
        );
        assert!(report.contains("# 510300"), "报告应包含 # 510300 标题");
    }
}
