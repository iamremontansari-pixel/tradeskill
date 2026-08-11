# Data Sources Catalog

Complete catalog of data sources used in the TradingAgents multi-agent analysis pipeline.

## US Market Sources

### Yahoo Finance (yfinance)
- **Provides**: Stock price data (OHLCV), company fundamentals, financial statements, news headlines
- **API Key**: Not required (free)
- **Markets**: US, HK, global exchanges
- **Used by**: Market Analyst, Fundamentals Analyst, News Analyst
- **Fallback**: Alpha Vantage (not wired into the `trad-skill` binary; agent uses web search fallback)

### Alpha Vantage
- **Provides**: Stock data, technical indicators, fundamental data, news
- **API Key**: Required (ALPHA_VANTAGE_API_KEY, free tier available)
- **Markets**: US primarily
- **Used by**: Market Analyst, Fundamentals Analyst, News Analyst (not wired into the `trad-skill` binary; agent uses web search fallback)
- **Fallback**: yfinance

### FRED (Federal Reserve Economic Data)
- **Provides**: Macroeconomic indicators (CPI, Core PCE, unemployment, fed funds rate, 10Y treasury, yield curve)
- **API Key**: Required (FRED_API_KEY, free)
- **Markets**: US macro
- **Used by**: News Analyst (macro context) (not wired into the `trad-skill` binary; agent uses web search fallback)
- **Indicators available**: cpi, core_pce, unemployment, fed_funds_rate, 10y_treasury, yield_curve

### Polymarket
- **Provides**: Prediction market probabilities for forward-looking events
- **API Key**: Not required (keyless)
- **Markets**: Global events
- **Used by**: News Analyst (not wired into the `trad-skill` binary; agent uses web search fallback)
- **Example queries**: "Fed rate cut", "recession 2026", geopolitical events

### StockTwits
- **Provides**: Retail trader sentiment messages with Bullish/Bearish tags
- **API Key**: Not required (public API)
- **Markets**: US stocks + Crypto (by cashtag)
- **Used by**: Sentiment Analyst
- **Signal**: Bullish/Bearish ratio as leading retail-sentiment indicator

### Reddit
- **Provides**: Community discussion posts with engagement metrics (upvotes, comments)
- **API Key**: Not required (public JSON API)
- **Subreddits**: r/wallstreetbets, r/stocks, r/investing
- **Used by**: Sentiment Analyst
- **Signal**: Engagement-weighted community attention

## China Market Sources (A股/港股)

### Tushare
- **Provides**: A股 daily/weekly/monthly data, financial statements, fundamentals
- **API Key**: Required (TUSHARE_TOKEN)
- **Markets**: China A-shares (Shanghai .SS, Shenzhen .SZ)
- **Used by**: China Market Analyst, Fundamentals Analyst (not wired into the `trad-skill` binary; agent uses web search fallback)
- **Priority**: Not wired — historical priority in the TradingAgents-CN fork only

### AKShare
- **Provides**: A股 real-time quotes, historical data, financial indicators, stock comments
- **API Key**: Not required (free, open source)
- **Markets**: China A-shares, HK stocks
- **Used by**: None in the trad-skill binary — the Rust binary calls the **Eastmoney** APIs directly (same underlying protocol AKShare wraps; see Implementation Notes). Listed here for historical provenance from the TradingAgents-CN fork.

### Baostock
- **Provides**: A股 historical K-line data, financial reports
- **API Key**: Not required (free)
- **Markets**: China A-shares
- **Used by**: China Market Analyst (fallback) (not wired into the `trad-skill` binary; agent uses web search fallback)
- **Priority**: Tertiary fallback

### TDX / 通达信
- **Provides**: Technical indicators, real-time quotes
- **API Key**: Not required (local data)
- **Markets**: China A-shares
- **Used by**: China Market Analyst (technical analysis) (not wired into the `trad-skill` binary; agent uses web search fallback)

### Google News (Chinese)
- **Provides**: Chinese financial news articles
- **API Key**: Not required
- **Markets**: A-shares, HK stocks
- **Used by**: CN News Analyst
- **Note**: Adds Chinese keywords (股票, 公司, 财报) for A-share queries

### Chinese Finance Social Sentiment
- **Provides**: Chinese social media sentiment analysis
- **API Key**: Not required
- **Markets**: A-shares
- **Used by**: Sentiment Analyst (CN) — the trad-skill binary fetches 千股千评 / 机构参与度 from the Eastmoney datacenter API (the semantic equivalent of the fork's akshare sources)

## China Fund Sources (公募基金/ETF/LOF)

All four fund endpoints are Eastmoney APIs, wired into the `trad-skill fund` subcommand. Agents may also hit them directly via web search/browser fallback. Historically returned GBK despite utf-8 headers; as of 2026-08-08 endpoints return UTF-8 (headers truthful). Decode UTF-8 first, fall back to GBK.

### Eastmoney Fund NAV History
- **Endpoint**: `api.fund.eastmoney.com/f10/lsjz` (JSON)
- **Params**: `fundCode`, `pageIndex`, `pageSize`, `startDate`, `endDate`, `callback=`
- **Fields**: FSRQ (date), DWJZ (unit NAV), LJJZ (cumulative NAV), JZZZL (NAV growth %), SGZT (subscription status), SHZT (redemption status)
- **Headers**: requires `Referer: https://fund.eastmoney.com/`; the `callback=` param must be present (JSONP wrapper)
- **Encoding**: GBK despite the utf-8 header

### Eastmoney Fund Profile (基金概况)
- **Endpoint**: `fundf10.eastmoney.com/jbgk_{code}.html` (HTML table)
- **Fields**: 基金全称, 类型, 成立日期, 规模, 经理, 管理人, 托管人
- **Encoding**: GBK

### Eastmoney Fund Holdings (重仓股)
- **Endpoint**: `fundf10.eastmoney.com/FundArchivesDatas.aspx?type=jjcc&code=<code>&topline=10&year=<year>` (JS+HTML)
- **Response shape**: `var apidata={content:"<escaped HTML table>"};`
- **Headers**: requires `Referer: https://fundf10.eastmoney.com/ccmx_{code}.html`
- **Encoding**: GBK

### Eastmoney Fund Performance (阶段涨幅)
- **Endpoint**: `fundf10.eastmoney.com/FundArchivesDatas.aspx?type=jdzf&code=<code>` (JS+HTML)
- **Response shape**: `var apidata={content:"<escaped HTML table>"};`
- **Headers**: requires `Referer: https://fundf10.eastmoney.com/jdzf_{code}.html`
- **Encoding**: GBK

**Rate limit**: ≤ 1 request/sec sustained; a single-call burst of 3-4 requests is acceptable.

## Data Source Degradation Chains

### A-Share Data (CN)
Eastmoney (default) → Yahoo Finance (via `stock --source yahoo`, symbol mapped to `.SS`/`.SZ`)

### US Stock Data
Yahoo Finance (default) → Eastmoney push2his (via `stock --source eastmoney`, for Yahoo-blocked regions)

### HK Stock Data
Eastmoney (default) → Yahoo Finance (via `stock --source yahoo`, symbol mapped to `.HK`)

### Crypto Data
Yahoo Finance only — Eastmoney does not support crypto, so there is no fallback channel.

### News (US)
Yahoo Finance + Google News (parallel)

### News (CN)
Eastmoney search API → Google News (Chinese)

### Fundamentals (US / A-share / HK)
US/Crypto: Yahoo Finance quoteSummary v10 + fundamentals-timeseries. A-share/HK: Eastmoney push2 + datacenter. No cross-channel fallback for fundamentals.

### A-Share Fund Data
Eastmoney only (no Yahoo fallback; Yahoo has no Chinese fund data).

## Configuration

> **Note**: This configuration system belongs to the original TradingAgents framework. This skill's binary does NOT read `data_vendors` / `tool_vendors` / API key env vars — all data sources are keyless public endpoints.

Data sources are configured via:
- Environment variables (API keys)
- `data_vendors` config dict (category-level vendor selection)
- `tool_vendors` config dict (tool-level override)

| Category | Default Vendor | Options |
|----------|---------------|---------|
| core_stock_apis | yfinance | alpha_vantage, yfinance |
| technical_indicators | yfinance | alpha_vantage, yfinance |
| fundamental_data | yfinance | alpha_vantage, yfinance |
| news_data | yfinance | alpha_vantage, yfinance |
| macro_data | fred | fred |
| prediction_markets | polymarket | polymarket |

## Implementation Notes

Data fetching is implemented in Rust (`trad-skill` binary) for all markets including US, HK, Crypto, and China A-shares.

### Rust Binary: trad-skill
- US stocks: Yahoo Finance v8 API (default; browser User-Agent + cookie/crumb handshake) — with an **Eastmoney push2his fallback channel** selectable via `stock --source eastmoney` (tries secid `105`/`106`/`107` = NASDAQ/NYSE/AMEX)
- HK stocks: Eastmoney push2his API (direct HTTP)
- Crypto: Yahoo Finance API (same as US stocks)
- Fundamentals: Yahoo Finance quoteSummary v10 (browser UA + cookie/crumb handshake) for US/Crypto; **Eastmoney** push2 + datacenter for A-shares **and HK stocks** (secid `116.{code}`; financial-indicator table gracefully degrades to "暂不可用" if Eastmoney has no HK rows)
- News: Yahoo Finance quoteSummary v10 (browser UA + cookie/crumb handshake) + Google News RSS in parallel (US); Eastmoney/Google News (CN)
- Sentiment: StockTwits + Reddit JSON API (US/Crypto); Eastmoney 千股千评/机构参与度 (A-shares); HK not supported
- China A-shares: Eastmoney APIs (via Rust HTTP) — stock, fundamentals, and news all auto-route here, no Yahoo dependency

### Data channel selection (`stock --source`)

`trad-skill stock` auto-selects the channel from the symbol: US/Crypto → Yahoo Finance, A-share/HK → Eastmoney. The `--source` flag overrides this:

| `--source` | US stocks | A-shares | HK stocks | Crypto |
|---|---|---|---|---|
| _(omitted, auto)_ | Yahoo | Eastmoney | Eastmoney | Yahoo |
| `yahoo` | Yahoo | Yahoo (`.SS`/`.SZ`) | Yahoo (`.HK`) | Yahoo |
| `eastmoney` | Eastmoney (105/106/107) | Eastmoney | Eastmoney | _not supported_ |

Use `--source eastmoney` for US stocks when Yahoo Finance is unreachable in your region (typical symptoms: `Yahoo Finance 错误(...): 未知错误` or HTTP 403 from datacenter/cloud IPs).
