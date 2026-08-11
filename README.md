# TradingAgents Analysis Skill

**English** | [中文](README_CN.md)

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

A multi-agent stock and crypto trading analysis skill for AI agents, inspired by [TradingAgents](https://github.com/TauricResearch/TradingAgents) and [TradingAgents-CN](https://github.com/hsliuping/TradingAgents-CN).

> **RESEARCH AND EDUCATION ONLY.** This skill does not constitute financial advice.

---

## Overview

This skill teaches an AI agent to replicate the TradingAgents multi-agent analysis pipeline. It orchestrates a team of specialized sub-agents to produce a structured, well-reasoned investment decision for any given ticker.

**Output**: one of five ratings (Buy / Overweight / Hold / Underweight / Sell) with detailed reasoning, key risks, and potential catalysts.

**Supported markets**:

- US stocks (AAPL, MSFT, TSLA...)
- China A-shares (600519, 000858...)
- HK stocks (0700.HK, 9988.HK...)
- Crypto (BTC-USD, ETH-USD...)

The pipeline runs in six stages:

1. **Analyst Team** (parallel sub-agents): Market, Sentiment, News, Fundamentals
2. **Research Debate**: Bull vs Bear researchers (structured argumentation, 1-3 rounds)
3. **Research Manager**: Judges the debate, produces an investment plan with a preliminary rating
4. **Trader**: Converts the plan into a concrete transaction proposal
5. **Risk Debate**: Aggressive vs Conservative vs Neutral analysts (1-3 rounds)
6. **Portfolio Manager**: Final decision with confidence level and full reasoning

---

## Installation

### Via `bunx` (recommended)

Install the skill with the built-in Rust installer. `bunx` (or `npx`) fetches the package and runs a thin launcher that resolves your platform binary and installs the skill into the **generic agent directory** `~/.agents/skills` by default:

```bash
bunx trad-skill@latest              # install to ~/.agents/skills (default)
```

Options:
```bash
bunx trad-skill@latest --agent claude      # install to ~/.claude/skills (Claude Code)
bunx trad-skill@latest --agent opencode    # install to ~/.config/opencode/skills
bunx trad-skill@latest --dir <path>        # install into a custom skills dir
bunx trad-skill@latest --dry-run           # print the install plan without writing
```

> `npx trad-skill@latest` behaves identically if you don't have `bun` installed. On older npx versions use `npx -p trad-skill@latest trad-skill`.

### Data tool without installing

The same binary also exposes `stock` / `news` / `fundamentals` / `sentiment` subcommands. Run them directly via `bunx` (or `npx`) without installing the skill — handy for one-off data fetches:

```bash
bunx trad-skill@latest stock --symbol AAPL
bunx trad-skill@latest news --symbol AAPL
bunx trad-skill@latest fundamentals --symbol AAPL
bunx trad-skill@latest sentiment --symbol AAPL
```

### For AI Agents

If you are an AI agent and need to install this skill, run:
```bash
bunx trad-skill@latest
```

Then restart or open a new session to load the skill. The skill will be available at:
- `~/.agents/skills/tradingagents-analysis` (default; OpenCode, Cline, Cursor, Windsurf, Codex, etc.)
- `~/.claude/skills/tradingagents-analysis` (Claude Code)
- `~/.config/opencode/skills/tradingagents-analysis` (OpenCode global)

### Deprecated: `npx skills add` (vercel-labs/skills CLI)

> **Deprecated.** The third-party `npx skills add halfoffive/trad-skill ...` flow (via [vercel-labs/skills](https://github.com/vercel-labs/skills)) is superseded by `bunx trad-skill@latest`. It still works for now but is no longer recommended and may be removed in a future release.

### Manual installation (using raw GitHub links)

Copy the skill files directly from the repository raw URLs.

**Claude Code (user-level):**
```bash
mkdir -p ~/.claude/skills/tradingagents-analysis
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/SKILL.md -o ~/.claude/skills/tradingagents-analysis/SKILL.md
mkdir -p ~/.claude/skills/tradingagents-analysis/references/prompts
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/data-sources.md -o ~/.claude/skills/tradingagents-analysis/references/data-sources.md
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/indicators.md -o ~/.claude/skills/tradingagents-analysis/references/indicators.md
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/prompts/README.md -o ~/.claude/skills/tradingagents-analysis/references/prompts/README.md
for f in market_analyst sentiment_analyst news_analyst fundamentals_analyst bull_researcher bear_researcher research_manager trader aggressive_risk conservative_risk neutral_risk portfolio_manager china_market_analyst cn_news_analyst; do
  curl -sL "https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/prompts/${f}.md" -o ~/.claude/skills/tradingagents-analysis/references/prompts/${f}.md
done
```

**Generic / OpenCode (user-level):** replace `~/.claude/skills` with `~/.agents/skills` in the commands above.

**Or simply clone/copy the directory:**
```bash
git clone --depth 1 https://github.com/halfoffive/trad-skill /tmp/trad-skill
cp -r /tmp/trad-skill/skills/tradingagents-analysis ~/.claude/skills/
rm -rf /tmp/trad-skill
```

---


## Usage

Trigger the skill by asking your AI agent to analyze a ticker:

- "Analyze AAPL for me"
- "What do you think about NVDA?"
- "Run a multi-agent analysis on BTC-USD"
- "Give me a trading analysis of 0700.HK"
- "Analyze 600519 with 3 debate rounds"
- "Analyze fund 000001 (华夏成长)"
- "Analyze ETF 510300 (沪深300ETF)"

The agent will orchestrate the full pipeline and produce a structured investment report ending with a summary table:

| Field | Value |
|---|---|
| Ticker | AAPL |
| Date | 2025-01-15 |
| Rating | Overweight |
| Confidence | medium |
| Market | US |

### China A-share notes

- Tickers use the 6-digit pure-number format (e.g., 600519, 000858)
- `trad-skill` auto-detects the exchange from the 6-digit code prefix (leading 6 → Shanghai .SS; leading 0/3 → Shenzhen .SZ), so you only need to provide the 6-digit number
- Data source: Eastmoney price APIs by default; use `trad-skill stock --source yahoo` to switch to the Yahoo channel (A-share codes are auto-mapped to .SS/.SZ)
- Chinese news and sentiment analysis are supported
- China-market-specific analyst prompts are used (`china_market_analyst.md`, `cn_news_analyst.md`)

### HK stock notes

- Use a 4-5 digit number + `.HK` suffix (e.g., 0700.HK or 00700.HK; `trad-skill` accepts both and zero-pads the code to 5 digits internally)
- Data source: HK prices default to the Eastmoney APIs (secid 116.<5-digit code>); `--source yahoo` switches to the Yahoo channel
- Stock Connect targets and HK main-board stocks are supported

### China A-share fund notes

- Fund codes are 6-digit (e.g. 000001), which COLLIDE with A-share stock codes (000001 平安银行). Use the `fund` subcommand to specify a fund: `trad-skill fund --symbol 000001`
- ETFs (510xxx/159xxx) are analyzable via BOTH `stock` (price-based) and `fund` (NAV-based)
- Data source: Eastmoney fund APIs only (no Yahoo fallback)

### Configuration

| Parameter | Range | Default | Description |
|---|---|---|---|
| `max_debate_rounds` | 1-3 | 1 | Bull/bear exchanges in the Research Debate |
| `max_risk_discuss_rounds` | 1-3 | 1 | Risk analyst exchanges in the Risk Debate |
| `output_language` | English / Chinese | match user | Language for all reports |
| `market` | auto-detect | — | Detected from ticker suffix |

Market auto-detection:

- 6-digit pure number (e.g., 600519, 000858) → A-shares
- 4/5-digit pure number (e.g., 0700, 09988) → HK stocks (when no `.HK` suffix)
- `.HK` suffix → HK stocks
- `-USD` suffix → Crypto
- Everything else → US stocks

---

## Architecture

```
+-----------------------------------------------------+
|                   ANALYST TEAM                       |
|  +----------+ +----------+ +--------+ +----------+  |
|  |  Market  | |Sentiment | |  News  | |Fundament.|  |
|  | Analyst  | | Analyst  | |Analyst | | Analyst  |  |
|  +----------+ +----------+ +--------+ +----------+  |
+-----------------------------------------------------+
                         |
                         v
+-----------------------------------------------------+
|              RESEARCH DEBATE (1-3 rounds)            |
|         Bull Researcher  <->  Bear Researcher        |
+-----------------------------------------------------+
                         |
                         v
              +---------------------+
              |  Research Manager   | -> Investment Plan
              +---------------------+
                         |
                         v
              +---------------------+
              |       Trader        | -> Transaction Proposal
              +---------------------+
                         |
                         v
+-----------------------------------------------------+
|              RISK DEBATE (1-3 rounds)                |
|   Aggressive  <->  Conservative  <->  Neutral       |
+-----------------------------------------------------+
                         |
                         v
              +---------------------+
              | Portfolio Manager   | -> FINAL DECISION
              +---------------------+
```

---

## Project Structure

```
trad-skill/                        # repo root (meta files + installer)
├── package.json                  # npm entry point (name: trad-skill)
├── bin/trad-skill.js             # thin JS launcher -> Rust binary (install + data)
├── README.md / README_CN.md       # bilingual docs with language switch
├── CHANGELOG.md                   # version history
├── AGENTS.md                      # AI-agent onboarding doc
├── LICENSE                        # Apache 2.0
├── .github/workflows/ci.yml      # CI: fmt + clippy + test + 7-platform build
├── crates/trad-data/              # Rust source (binary name: trad-skill)
└── skills/
    └── tradingagents-analysis/    # the installable skill
        ├── SKILL.md               # Core skill instructions (pipeline, orchestration, output format)
        └── references/
            ├── prompts/           # 14 verbatim agent role prompts
            │   ├── market_analyst.md
            │   ├── sentiment_analyst.md
            │   ├── news_analyst.md
            │   ├── fundamentals_analyst.md
            │   ├── bull_researcher.md
            │   ├── bear_researcher.md
            │   ├── research_manager.md
            │   ├── trader.md
            │   ├── aggressive_risk.md
            │   ├── conservative_risk.md
            │   ├── neutral_risk.md
            │   ├── portfolio_manager.md
            │   ├── china_market_analyst.md
            │   ├── cn_news_analyst.md
            │   └── README.md
            ├── data-sources.md    # Data source catalog (US + CN markets)
            └── indicators.md      # Technical indicator reference
```

---

## Data Sources

| Source | Market | Provides | API Key |
|--------|--------|----------|---------|
| Yahoo Finance | US/Global | Price, fundamentals, news | Free |
| Alpha Vantage | US | Price, indicators, fundamentals | Free tier |
| FRED | US Macro | CPI, unemployment, rates | Free |
| Polymarket | Global | Prediction probabilities | Free |
| StockTwits | US | Retail sentiment | Free |
| Reddit | US | Community discussion | Free |
| Tushare | A-shares | Price, fundamentals | Token |
| AKShare | A-shares/HK | Price, news, sentiment | Free |
| Baostock | A-shares | Historical data | Free |
| TDX (通达信) | A-shares | Technical indicators | Free |

> Note: Alpha Vantage, FRED, Polymarket, Tushare, Baostock, and TDX are listed for historical/provenance purposes; the trad-skill binary uses Yahoo Finance + Eastmoney directly. See [references/data-sources.md](skills/tradingagents-analysis/references/data-sources.md) for the wired-in sources and fallback strategies.

See [references/data-sources.md](skills/tradingagents-analysis/references/data-sources.md) for the full catalog with fallback strategies.

---

## Data Tool (`trad-skill`)

Data is fetched by the `trad-skill` Rust binary, which provides market data (OHLCV + indicators), news, fundamentals, and sentiment in a single compact output suitable for LLM prompt injection. Platform binaries are distributed as the five `@trad-skill/<platform>` npm packages (installed as optional dependencies), and the thin launcher `bin/trad-skill.js` resolves the correct one at runtime. The same binary handles both install and data subcommands.

> The agent must run `trad-skill` by its **absolute path** inside the installed skill directory (e.g. `~/.agents/skills/tradingagents-analysis/bin/<platform>/trad-skill`), because a sub-agent's working directory is the user's project, not the skill folder. The skill's `SKILL.md` instructs the main agent to resolve that path before spawning analysts.

```bash
# Stock data: OHLCV tail + pre-computed indicators + optional stats
trad-skill stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats

# Or run directly via bunx (no install needed):
bunx trad-skill@latest stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats

# Or by absolute path (how the agent invokes them):
~/.agents/skills/tradingagents-analysis/bin/<platform>/trad-skill stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats

# China A-share: use the 6-digit code directly (auto-routed to Eastmoney)
trad-skill stock --symbol 600519 --tail 30

# Pick a data channel explicitly — e.g. route US stocks via Eastmoney when Yahoo
# Finance is blocked in your region (returns "未知错误" / 403):
trad-skill stock --symbol AAPL --source eastmoney

# Fetch news (default --limit 8 per source, summaries truncated)
trad-skill news --symbol AAPL --days 7 --limit 8

# Fetch fundamentals (compact key-metrics table + company profile)
trad-skill fundamentals --symbol AAPL

# Fetch sentiment (default --limit 15, --days 7 Reddit window)
trad-skill sentiment --symbol AAPL --limit 15 --days 7

# China A-share & HK fundamentals/news auto-route to Eastmoney (no Yahoo required):
trad-skill fundamentals --symbol 600519
trad-skill news --symbol 600519
trad-skill stock --symbol 0700.HK        # HK OHLCV via Eastmoney
trad-skill news --symbol 0700.HK         # HK news via Eastmoney
# HK sentiment is not supported (StockTwits/Reddit are US-only; Eastmoney
# 千股千评 does not cover HK) — use the US-listed ticker if available.
```

| Subcommand | Defaults (compact) | Expand flags |
|---|---|---|
| `stock` | `--tail 30` + `--indicators` on | `--stats`, `--raw`, `--source yahoo\|eastmoney` |
| `news` | `--limit 8`, 200-char summaries | `--limit N`, `--days N` |
| `fundamentals` | compact key-metrics table | — |
| `sentiment` | `--limit 15`, `--days 7` (Reddit), 8 messages/posts shown | `--limit N`, `--days N` |
| `fund` | 公募基金/ETF/LOF：净值历史 + 基金资料 + 重仓股 + 业绩表现（东方财富） | `trad-skill fund --symbol 000001 --tail 30` |

`stock --source` selects the data channel: by default US/Crypto use Yahoo Finance and A-shares/HK use Eastmoney, auto-detected from the symbol. Pass `--source eastmoney` to route US stocks through Eastmoney (handy when Yahoo is region-blocked), or `--source yahoo` to force Yahoo (A-share/HK symbols are mapped to `.SS`/`.SZ`/`.HK`). Eastmoney does not serve crypto.

**When Yahoo Finance is unreachable** (symptom: `未知错误`, `401 Unauthorized`, or `403 Forbidden` from datacenter/cloud IPs): A-share `stock` / `fundamentals` / `news` already use Eastmoney automatically — just pass the 6-digit code. For US price data, add `--source eastmoney`. US `fundamentals` / `news` have no Eastmoney equivalent, so the agent falls back to web search for those parts.

`trad-skill` is the **primary** data source and is tried first. It is not a hard dependency in the sense that, if a subcommand errors or a source is unavailable, the agent falls back to web search / browser tools **only for the parts the command could not provide** — it never skips the binary outright.

---

## Credits

- **[TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents)**: Original multi-agent trading framework (Apache 2.0)
- **[hsliuping/TradingAgents-CN](https://github.com/hsliuping/TradingAgents-CN)**: China market enhancements, A-share and HK stock support
- **Paper**: Xiao et al., "TradingAgents: Multi-Agents LLM Financial Trading Framework", [arXiv:2412.20138](https://arxiv.org/abs/2412.20138)

---

## Disclaimer

> **This skill is for RESEARCH AND EDUCATION ONLY.**
>
> - Nothing produced by this skill constitutes financial, investment, or trading advice.
> - Past performance does not guarantee future results.
> - LLM-generated analysis is non-deterministic and may contain factual errors, hallucinated data, or flawed reasoning.
> - Always consult a qualified, licensed financial advisor before making any investment decision.
> - The authors and contributors accept no liability for any losses incurred from acting on this output.

---

## License

[Apache License 2.0](LICENSE)
