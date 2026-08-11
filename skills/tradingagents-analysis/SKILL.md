---
name: tradingagents-analysis
description: "Multi-agent stock/crypto trading analysis pipeline inspired by TradingAgents. Orchestrates specialized analyst sub-agents (Market, Sentiment, News, Fundamentals), structured bull/bear research debates, risk assessment debates, and a final portfolio decision. Use when the user asks for stock analysis, trading analysis, multi-agent financial research, investment research, ticker analysis, or says '分析股票', '交易分析', '多智能体分析', '投资研究'. Also trigger on any request to analyze a specific ticker symbol (AAPL, 600519, 0700.HK, BTC-USD, etc.) with a trading/investment lens."
---

## For AI Agents: Installation Guide

If this skill is not yet installed and you need to use it, install it with the built-in Rust installer (default target: `~/.agents/skills`):

```bash
bunx trad-skill@latest
```

Options: `--agent claude` (→ `~/.claude/skills`) or `--agent opencode` (→ `~/.config/opencode/skills`), or `--dir <path>` for a custom location. `npx trad-skill@latest` works identically as a fallback.

> The legacy third-party `npx skills add halfoffive/trad-skill ...` flow is deprecated; prefer `bunx trad-skill@latest`.

After installation, the skill will be available at one of these locations (check in order):
- `~/.agents/skills/tradingagents-analysis` (default; OpenCode, Cline, Cursor, Windsurf, Codex, etc.)
- `~/.claude/skills/tradingagents-analysis` (Claude Code)
- `~/.config/opencode/skills/tradingagents-analysis` (OpenCode global)

Data tool (Rust binary):
  The same `trad-skill` binary powers both the installer and data fetching. It provides stock, fundamentals, news, and sentiment data for US stocks, China A-shares, HK stocks, and Crypto. Sentiment coverage: StockTwits/Reddit for US/Crypto, Eastmoney 千股千评/机构参与度 for A-shares; HK sentiment is not supported. If the bundled binary is missing, run any subcommand directly via `bunx trad-skill@latest <subcommand>` (e.g. `bunx trad-skill@latest stock --symbol AAPL`).

Verify installation by checking that `SKILL.md` and `references/` exist in the skill directory.

---

# TradingAgents Multi-Agent Analysis

## 1. Overview

This skill replicates the TradingAgents multi-agent pipeline, drawing on the open-source frameworks from TauricResearch/TradingAgents and hsliuping/TradingAgents-CN. It orchestrates a team of specialized sub-agents to produce a structured, well-reasoned investment decision for any given ticker.

**Output**: one of five ratings — Buy / Overweight / Hold / Underweight / Sell — accompanied by detailed reasoning, key risks, and potential catalysts.

**Supported markets**:

- US stocks (AAPL, MSFT, TSLA…)
- China A-shares / A股 (600519, 000858…)
- HK stocks / 港股 (0700.HK, 9988.HK…)
- Crypto (BTC-USD, ETH-USD…)

> **RESEARCH AND EDUCATION ONLY.** Nothing this skill produces constitutes financial advice. See the Disclaimer section below.

---

## 2. Before You Start — Confirm the Target

**Do not spawn any sub-agent until the analysis target is confirmed.**

1. If the user has **not** named a specific ticker/symbol, **ask them first**: "Which ticker should I analyze?" Give market-aware examples (e.g. `AAPL`, `600519` / `000858`, `0700.HK`, `BTC-USD`). Wait for the answer before proceeding.
2. Optionally confirm — or let the defaults apply — the following:

   | Parameter | Default | Notes |
   |---|---|---|
   | Trade date | today | The "as of" date for the analysis |
   | `max_debate_rounds` | 1 | 1–3 bull/bear exchanges |
   | `max_risk_discuss_rounds` | 1 | 1–3 risk-debate exchanges |
   | `output_language` | match the user's language | English or 中文 |
   | `market` | auto-detect from ticker | 6 位纯数字（如 600519、000858）→A股, 4/5 位纯数字（如 0700、09988）→港股, `.HK`→港股, `-USD`→Crypto, else US |

3. The user may override any default inline (e.g. "用中文输出", "run 3 debate rounds"). Honor stated preferences.
4. Only once a ticker is confirmed, proceed to the pipeline below.

> **基金代码与股票代码冲突**：6 位基金代码（如 000001 华夏成长）与 A 股股票代码（000001 平安银行）冲突。当目标是基金时，用 `fund` 子命令显式指定：`trad-skill fund --symbol 000001`。`detect_market` 不自动识别基金——子命令名本身携带类型。

---

## 3. Pipeline Architecture

The analysis runs as a six-stage pipeline. Stage 1 uses parallel sub-agents; stages 2–6 run sequentially in the main context.

### Stage 1 — Analyst Team (PARALLEL)

Four sub-agents run simultaneously, each producing a structured report:

| Analyst | Focus | Key inputs |
|---|---|---|
| **Market Analyst** | Technical indicators: SMA, EMA, MACD, RSI, Bollinger Bands, ATR, VWMA, MFI | OHLCV + **pre-computed indicators** via `trad-skill stock` (analyst interprets, does not recompute) |
| **Sentiment Analyst** | Social sentiment → composite score | StockTwits, Reddit (US) via `trad-skill sentiment` |
| **News Analyst** | Company news and macro context | Company news via `trad-skill news` (FRED / Polymarket / macro: web-search fallback) |
| **Fundamentals Analyst** | Financial statements: balance sheet, cashflow, income statement | Financials via `trad-skill fundamentals` |

### Stage 2 — Research Debate (SEQUENTIAL, 1–3 rounds)

- **Bull Researcher** argues FOR investing, citing all four analyst reports.
- **Bear Researcher** argues AGAINST investing, citing the same reports.
- They alternate for `max_debate_rounds` exchanges (default: 1).

### Stage 3 — Research Manager

Judges the debate and produces a structured investment plan with a preliminary rating.

### Stage 4 — Trader

Converts the investment plan into a concrete transaction proposal (entry/exit levels, position sizing guidance, time horizon).

### Stage 5 — Risk Debate (SEQUENTIAL, 1–3 rounds)

Three risk perspectives alternate:

- **Aggressive Analyst**: champions high-reward strategies.
- **Conservative Analyst**: prioritizes capital preservation.
- **Neutral Analyst**: balances risk and reward.

They exchange for `max_risk_discuss_rounds` rounds (default: 1).

### Stage 6 — Portfolio Manager

Reviews the trader's proposal and all risk perspectives. Issues the **final decision**: Buy / Overweight / Hold / Underweight / Sell, with confidence level and full reasoning.

---

## 4. Sub-Agent Orchestration

### Spawning the Analyst Team

Spawn **four parallel sub-agents**, one per analyst role. Each sub-agent receives:

1. The **ticker symbol** and **trade date** (default: today).
2. Its **role prompt**, loaded verbatim from the corresponding file in `references/prompts/`:
   - `references/prompts/market_analyst.md`
   - `references/prompts/sentiment_analyst.md`
   - `references/prompts/news_analyst.md`
   - `references/prompts/fundamentals_analyst.md`

   > **CN market prompt swap.** 当 `market` 为 A 股或港股时，用 `references/prompts/china_market_analyst.md` 替换 `market_analyst.md`，用 `references/prompts/cn_news_analyst.md` 替换 `news_analyst.md`；其余 2 个分析师（Sentiment / Fundamentals）保持不变。Stage 2 及之后的 researcher / manager / risk debator 等角色不受 `market` 影响（其 prompt 通用，不区分市场）。
   >
   > 当 `market` 为 A 股基金时，用 `references/prompts/fund_market_analyst.md` 替换 `market_analyst.md`，用 `fund_sentiment_analyst.md` 替换 `sentiment_analyst.md`，用 `fund_news_analyst.md` 替换 `news_analyst.md`，用 `fund_fundamentals_analyst.md` 替换 `fundamentals_analyst.md`；Stage 2 及之后角色不受影响。
3. The **absolute path** to the `trad-skill` binary (see Section 6).

### Resolve the skill directory first (important)

A sub-agent's working directory is the user's project, **not** this skill's folder. Before spawning anything, **resolve the absolute path to this skill's `bin/` directory** and embed it in each sub-agent prompt.

Locate the skill directory — it is typically one of:

- `~/.claude/skills/tradingagents-analysis` (Claude Code, user-level)
- `~/.agents/skills/tradingagents-analysis` (generic / OpenCode user-level)
- `.claude/skills/tradingagents-analysis` or `.opencode/skills/tradingagents-analysis` (project-level)

Set `SKILL_DIR` to that path and use it in every spawn. The platform binary lives at `{SKILL_DIR}/bin/<platform>/trad-skill[.exe]`, where `<platform>` is one of `win32-x64`, `win32-arm64`, `darwin-arm64`, `linux-x64`, `linux-arm64` (matching Node's `process.platform`-`process.arch`).

### Spawn template

Use background task spawning for parallelism:

```
task(subagent_type="general", run_in_background=true,
     prompt="<role prompt contents>\n\nAnalyze {ticker} as of {date}.\n" +
            "Gather data FIRST by running the binary, then write your report.\n" +
            "Run: \"{SKILL_DIR}/bin/{platform}/trad-skill[.exe]\" {subcommand} --symbol {ticker} ...\n" +
            "or (if no bundled binary is available): bunx trad-skill@latest {subcommand} --symbol {ticker} ...\n" +
            "The binary output is ALREADY compact and (for market data) pre-computes indicators — " +
            "it is your data source AND your verified snapshot; do NOT call get_stock_data / " +
            "get_indicators / get_verified_market_snapshot or any other tool name, they do not exist. " +
            "Do NOT copy raw binary output into your report — cite the key numbers only. " +
            "If the binary errors, fall back to web search / browser tools only for the parts it could not provide.")
```

Substitute the correct subcommand **and** args per analyst (see Section 6). **`trad-skill` is the primary data source** — the analyst must run its assigned subcommand before writing its report; web search / browser tools are a fallback only when the binary fails or returns no data for a given source.

> **Template variables in verbatim prompts.** The role prompts in `references/prompts/` contain LangChain-style variables (`{ticker}`, `{instrument_context}`, `{get_language_instruction()}`, `{NO_EXTERNAL_TOOLS}`, and ~24 others — 30 in total; of which `{current_date}`/`{tool_names}`/`{system_message}` are phantom no-ops, and for funds `{target_label}`=fund/`{asset_label}`=fund/`{fundamentals_label}`=Fund fundamentals report per prompts/README §Template Variable Substitution). **Substitute them before spawning** per the table in `references/prompts/README.md` (§ "Template Variable Substitution"). Quick reference: `{ticker}` → ticker; `{target_label}` → `stock` (equities) or `asset` (crypto); `{asset_label}` → `company` (equities) or `asset` (crypto); `{fundamentals_label}` → `Company fundamentals report` (equities) or `Asset fundamentals report (may be unavailable for crypto)` (crypto); `{current_date}` → today; `{start_date}`/`{end_date}` → analysis window; `{instrument_context}` → `Market: <US/A股/港股/Crypto>; Ticker: <symbol>; Trade date: <date>`; `{get_language_instruction()}` → empty string (English) or ` Write your entire response in <lang>.` (non-English); `{tool_names}`/`{system_message}`/`{lessons_line}` → empty string; `{NO_EXTERNAL_TOOLS}` → empty (not set — fallback permitted); data-report variables (`{market_research_report}` etc.) → bound to stage outputs per "Re-injection discipline" below.

> **Token discipline.** Each analyst report must be **concise (target ≤ 400 words)** and structured as: a `## Key Signals` block of 5–8 actionable bullets at the top, a short evidence section referencing the binary's key numbers (not reproducing the raw output), and one summary table. The `## Key Signals` block is what downstream stages consume — keep it self-contained. This overrides any "very detailed" phrasing in the verbatim role prompt: be evidence-dense, not verbose.

Wait for all four analysts to complete before proceeding.

### Running Sequential Stages

Stages 2–6 run **in the main context**, one after another. For each stage:

1. Load the role prompt from `references/prompts/{role}.md`.
2. Feed in all prior stage outputs as context.
3. Collect the structured output.
4. Pass it forward to the next stage.

Debate stages (2 and 5) loop for the configured number of rounds. Each round, the next speaker receives the full transcript of prior rounds.

> **Re-injection discipline (biggest token lever).** The verbatim role prompts bind the four analyst reports via template variables (`{market_research_report}`, `{sentiment_report}`, `{news_report}`, `{fundamentals_report}`). To avoid re-sending four full reports on every debate round:
> - **Stages 2 & 5** (debates): bind those four variables to each report's **`## Key Signals` digest only**, not the full body. The `{history}` variable still carries the running debate transcript (naturally paragraph-sized, not full reports).
> - **Stage 6** (Portfolio Manager): `portfolio_manager.md` does **not** define `{market_research_report}` / `{sentiment_report}` / `{news_report}` / `{fundamentals_report}` slots in its body — only `{research_plan}`, `{trader_plan}`, `{history}` (risk debate transcript), and `{lessons_line}`. So bind `{research_plan}` to the Research Manager plan, `{trader_plan}` to the Trader proposal, `{history}` to the full risk-debate transcript, and **append the four full analyst reports as out-of-template context** (e.g., prepend them to the prompt as a `## Analyst Reports` section). The final synthesis deserves complete context, and it happens a single time.
> Extract the `## Key Signals` block from each analyst report before feeding it into the debate prompts; keep the full reports aside for the Portfolio Manager.

---

## 5. Stage-by-Stage Instructions

### Stage 1: Analyst Team

- **Data**: each analyst runs its assigned `trad-skill` subcommand (see Section 6).
- **Prompt**: `references/prompts/{role}_analyst.md`.
- **Output**: a **concise** structured markdown report (≤ ~400 words) leading with a `## Key Signals` digest (5–8 bullets), followed by a short evidence section and one summary table. Do not reproduce the raw output — cite key numbers only.
- **Handoff**: keep each full report aside for the Portfolio Manager; extract the four `## Key Signals` digests into one context block for the Stage 2 & 5 debates (see "Re-injection discipline" above).

### Stage 2: Research Debate

- **Data**: the four analyst reports from Stage 1.
- **Prompts**: `references/prompts/bull_researcher.md` and `references/prompts/bear_researcher.md`.
- **Output**: alternating argument paragraphs, each citing specific data points from the analyst reports.
- **Handoff**: the full debate transcript goes to the Research Manager.

### Stage 3: Research Manager

- **Data**: the debate transcript.
- **Prompt**: `references/prompts/research_manager.md`.
- **Output**: a structured investment plan including preliminary rating, thesis, and conditions.
- **Handoff**: the plan goes to the Trader.

### Stage 4: Trader

- **Data**: the investment plan.
- **Prompt**: `references/prompts/trader.md`. **Note (R6-25):** `trader.md` has separate `## System Message` and `## User Message` code blocks — construct the LLM call with both roles (system message = the System Message block, user message = the User Message block with `{research_plan}` / `{lessons_line}` substituted). Do not concatenate them into a single prompt.
- **Output**: a transaction proposal with entry/exit levels, position sizing, and time horizon.
- **Handoff**: the proposal goes to the Risk Debate.

### Stage 5: Risk Debate

- **Data**: the transaction proposal plus all analyst reports.
- **Prompts**: `references/prompts/aggressive_risk.md`, `references/prompts/conservative_risk.md`, `references/prompts/neutral_risk.md`.
- **Output**: alternating risk perspectives for `max_risk_discuss_rounds` rounds.
- **Handoff**: the risk transcript goes to the Portfolio Manager.

### Stage 6: Portfolio Manager

- **Data**: everything. All analyst reports, the debate, the plan, the proposal, and the risk transcript.
- **Prompt**: `references/prompts/portfolio_manager.md`.
- **Output**: the final decision in the format described in Section 7.

---

## 6. Data Gathering

The `trad-skill` Rust binary lives in this skill's `bin/<platform>/` directory. It fetches, **compacts, and pre-computes** data for the analyst sub-agents — so the analyst interprets a small payload instead of burning tokens on raw data and arithmetic. **Run it with its absolute path** (see the "Resolve the skill directory first" note in Section 4); each subcommand prints a formatted string ready for prompt injection. If the bundled binary is missing, the equivalent `bunx trad-skill@latest <subcommand>` works without an install.

| Command | Purpose | Invocation |
|---|---|---|
| `trad-skill stock` | OHLCV **tail** (default 30 rows) + **pre-computed indicators** (SMA/EMA/MACD/RSI/Bollinger/ATR/VWMA/MFI) + optional stats. The Market Analyst **interprets** these pre-computed values (no manual arithmetic). (defaults when omitted: `--end` = today, `--start` = today − 365 days) | `trad-skill stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats` |
| `trad-skill news` | Company news (US: Yahoo Finance + Google News RSS). Default `--limit 8` per source; all summaries truncated. | `trad-skill news --symbol AAPL --days 7 --limit 8` |
| `trad-skill fundamentals` | **Compact key-metrics table** (revenue, net income, EPS, FCF, debt, margins, YoY) + company profile — instead of dumping full 4-year statements. | `trad-skill fundamentals --symbol AAPL` |
| `trad-skill sentiment` | Social sentiment from StockTwits, Reddit. Default `--limit 15`, `--days 7` (Reddit window); message/post displays trimmed. | `trad-skill sentiment --symbol AAPL --limit 15 --days 7` |
| `trad-skill fund` | 公募基金/ETF/LOF：净值历史 + 基金资料 + 重仓股 + 业绩表现（东方财富 fund API）。默认 `--tail 30`（净值历史尾行），`--days 365`（净值日期窗）。 | `trad-skill fund --symbol 000001 --tail 30` |

> **China A-share market**: `trad-skill` supports A-share data via Eastmoney APIs. Use 6-digit symbols (e.g. `600519`) directly — e.g. `trad-skill stock --symbol 600519 --tail 30`.
>
> **Data channel (`--source yahoo|eastmoney`)**: `stock` auto-selects the channel from the symbol (US/Crypto → Yahoo Finance, A-share/HK → Eastmoney). For the full channel-selection table and `--source` override semantics (incl. the Yahoo-blocked-region fallback for US stocks), see `references/data-sources.md` → "Data channel selection".
>
> **A股优先东方财富；Yahoo 不可达时的回退**: A股的 `stock` / `fundamentals` / `news` 全部自动走东方财富（不依赖 Yahoo），直接用 6 位代码即可——A股分析应首选东方财富源。当 Yahoo Finance 不可达（症状：`未知错误` / `401 Unauthorized` / `403 Forbidden`，常见于数据中心/云 IP）时：美股行情改用 `stock --source eastmoney`；美股 `fundamentals` / `news` 没有东方财富对应源，**仅对这两部分**回退到网络搜索/浏览器工具，其余子命令仍走 `trad-skill`。
>
> **港股 fundamentals 自动走东方财富**: 港股（`0700.HK`、`09988` 等）的 `fundamentals` 子命令自动路由到东方财富（secid `116.{code}`），无需 `--source` 参数，与 A股行为一致。Yahoo Finance 不可达时港股基本面不受影响；若东方财富 datacenter 无对应财务指标行，基本面报告仍会输出个股基本信息，财务指标表优雅降级为「暂不可用」。
>
> **HK sentiment is not supported** (StockTwits/Reddit are US-only; Eastmoney 千股千评 does not cover HK) — use the US-listed ticker if available (e.g. 9988.HK -> BABA).

For the full catalog of data sources, APIs, and fallback strategies, see `references/data-sources.md`.

For technical indicator definitions and interpretation guidance, see `references/indicators.md`.

> `trad-skill` is the **primary** data source and must be tried first. If the binary errors or a source is unavailable, the agent falls back to web search / browser tools **only for the parts it could not provide** — never skip it outright.

---

## 7. Output Format

The final Portfolio Manager decision must include all of the following:

- **Rating**: exactly one of `Buy` / `Overweight` / `Hold` / `Underweight` / `Sell`.
- **Confidence**: `low` / `medium` / `high`.
- **Reasoning**: a **concise 3–4 paragraph** synthesis drawing on all analyst reports, the research debate, and the risk debate. Explain why the rating was chosen and what evidence supports it — cite specific data points, do not re-narrate the full pipeline.
- **Key Risks**: bullet list of the most significant downside scenarios.
- **Catalysts**: bullet list of events or conditions that could move the thesis.

Close with a summary table:

```markdown
| Field | Value |
|---|---|
| Ticker | {ticker} |
| Date | {analysis_date} |
| Rating | {rating} |
| Confidence | {confidence} |
| Market | {US / A股 / 港股 / Crypto} |
```

---

## 8. Configuration

| Parameter | Range | Default | Description |
|---|---|---|---|
| `max_debate_rounds` | 1–3 | 1 | Number of bull/bear exchanges in the Research Debate |
| `max_risk_discuss_rounds` | 1–3 | 1 | Number of risk analyst exchanges in the Risk Debate |
| `output_language` | English / 中文 | match user's language | Language for all reports and the final decision |
| `market` | auto-detect | — | Detected from ticker suffix (see below) |

**Market auto-detection rules**:

- 6 位纯数字（如 600519、000858） → A股 (China A-shares)
- 4/5 位纯数字（如 0700、09988） → 港股 (HK stocks, 无 .HK 后缀时)
- Suffix `.HK` → 港股 (HK stocks)
- Suffix `-USD` → Crypto
- Everything else → US stocks
- 基金（公募基金/ETF/LOF）不通过 `detect_market` 自动识别（代码与股票冲突）；用 `fund` 子命令显式指定。

The user can override any of these by stating preferences explicitly (e.g., "用中文输出", "run 3 debate rounds").

---

## 9. Disclaimer

> **This skill is for RESEARCH AND EDUCATION ONLY.**
>
> - Nothing produced by this skill constitutes financial, investment, or trading advice.
> - Past performance does not guarantee future results.
> - LLM-generated analysis is non-deterministic and may contain factual errors, hallucinated data, or flawed reasoning.
> - Always consult a qualified, licensed financial advisor before making any investment decision.
> - The authors and contributors of this skill accept no liability for any losses incurred from acting on its output.

---

## 10. Credits

- Based on [TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents) (Apache 2.0).
- China market enhancements from [hsliuping/TradingAgents-CN](https://github.com/hsliuping/TradingAgents-CN).
- Paper: "TradingAgents: Multi-Agents LLM Financial Trading Framework" ([arXiv:2412.20138](https://arxiv.org/abs/2412.20138)).
