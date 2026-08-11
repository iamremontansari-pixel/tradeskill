# TradingAgents 多智能体分析技能

[English](README.md) | **中文**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

基于 [TradingAgents](https://github.com/TauricResearch/TradingAgents) 和 [TradingAgents-CN](https://github.com/hsliuping/TradingAgents-CN) 的多智能体股票/加密货币分析 AI 技能。

> **仅供研究与学习使用** — 本技能不构成任何投资建议。

---

## 概述

本技能教会 AI 代理复现 TradingAgents 多智能体分析流水线：

1. **分析师团队**（并行子代理）：市场分析师、情绪分析师、新闻分析师、基本面分析师
2. **研究辩论**：看多研究员 vs 看空研究员（结构化辩论）
3. **研究主管**：评判辩论 → 生成带评级的投资计划
4. **交易员**：将计划转化为具体交易提案
5. **风控辩论**：激进派 vs 保守派 vs 中性派
6. **投资组合经理**：最终决策（买入/增持/持有/减持/卖出）

**支持市场**：

- 美股（AAPL, MSFT, TSLA…）
- A股 / 中国A股（600519, 000858…）
- 港股（0700.HK, 9988.HK…）
- 加密货币（BTC-USD, ETH-USD…）

---

## 安装

### 通过 bunx 安装（推荐）

使用内置的 Rust 安装器安装技能。`bunx`（或 `npx`）拉取包并运行一个极简的 launcher，它解析当前平台的二进制并把技能安装到**通用 agent 目录** `~/.agents/skills`（默认）：

```bash
bunx trad-skill@latest              # 安装到 ~/.agents/skills（默认）
```

可选参数：
```bash
bunx trad-skill@latest --agent claude      # 安装到 ~/.claude/skills（Claude Code）
bunx trad-skill@latest --agent opencode    # 安装到 ~/.config/opencode/skills
bunx trad-skill@latest --dir <path>        # 安装到自定义技能目录
bunx trad-skill@latest --dry-run           # 只打印安装计划，不写入
```

> 若未安装 `bun`，`npx trad-skill@latest` 行为完全一致。旧版 npx 可用 `npx -p trad-skill@latest trad-skill`。

### 免安装直接使用数据工具

同一个二进制还暴露 `stock` / `news` / `fundamentals` / `sentiment` / `fund` 子命令。无需安装技能即可通过 `bunx`（或 `npx`）直接运行，适合一次性取数：

```bash
bunx trad-skill@latest stock --symbol AAPL
bunx trad-skill@latest news --symbol AAPL
bunx trad-skill@latest fundamentals --symbol AAPL
bunx trad-skill@latest sentiment --symbol AAPL
```

### 给 AI 代理的说明

如果你是 AI 代理需要安装本技能，请运行：
```bash
bunx trad-skill@latest
```

安装后重启或开启新会话以加载技能。技能将安装在以下位置之一：
- `~/.agents/skills/tradingagents-analysis`（默认；OpenCode、Cline、Cursor、Windsurf、Codex 等）
- `~/.claude/skills/tradingagents-analysis`（Claude Code）
- `~/.config/opencode/skills/tradingagents-analysis`（OpenCode 全局）

### 已弃用：`npx skills add`（vercel-labs/skills CLI）

> **已弃用。** 第三方 `npx skills add halfoffive/trad-skill ...` 流程（基于 [vercel-labs/skills](https://github.com/vercel-labs/skills)）已被 `bunx trad-skill@latest` 取代。当前仍可用，但不再推荐，未来版本可能移除。

### 手动安装（使用 raw GitHub 链接直接复制）

直接从仓库的 raw URL 复制技能文件。

**Claude Code（用户级）：**
```bash
mkdir -p ~/.claude/skills/tradingagents-analysis
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/SKILL.md -o ~/.claude/skills/tradingagents-analysis/SKILL.md
mkdir -p ~/.claude/skills/tradingagents-analysis/references/prompts
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/data-sources.md -o ~/.claude/skills/tradingagents-analysis/references/data-sources.md
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/indicators.md -o ~/.claude/skills/tradingagents-analysis/references/indicators.md
curl -sL https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/prompts/README.md -o ~/.claude/skills/tradingagents-analysis/references/prompts/README.md
for f in market_analyst sentiment_analyst news_analyst fundamentals_analyst bull_researcher bear_researcher research_manager trader aggressive_risk conservative_risk neutral_risk portfolio_manager china_market_analyst cn_news_analyst fund_market_analyst fund_sentiment_analyst fund_news_analyst fund_fundamentals_analyst; do
  curl -sL "https://raw.githubusercontent.com/halfoffive/trad-skill/main/skills/tradingagents-analysis/references/prompts/${f}.md" -o ~/.claude/skills/tradingagents-analysis/references/prompts/${f}.md
done
```

**通用 / OpenCode（用户级）：** 将上面命令中的 `~/.claude/skills` 替换为 `~/.agents/skills`。

**或者直接 clone 后复制目录：**
```bash
git clone --depth 1 https://github.com/halfoffive/trad-skill /tmp/trad-skill
cp -r /tmp/trad-skill/skills/tradingagents-analysis ~/.claude/skills/
rm -rf /tmp/trad-skill
```

---

## 使用

向你的 AI 代理请求分析某个标的即可触发：

- "分析一下苹果公司 AAPL"
- "帮我看看英伟达 NVDA 怎么样"
- "分析贵州茅台 600519"
- "用多智能体分析一下比特币 BTC-USD"
- "帮我分析港股腾讯 0700.HK"
- "交易分析：特斯拉 TSLA"
- "投资研究：比亚迪 002594"
- "分析基金 000001（华夏成长）"
- "分析ETF 510300（沪深300ETF）"

代理将编排完整的分析流水线，生成结构化的投资研究报告，报告末尾附一张汇总表：

| 字段 | 值 |
|---|---|
| 代码 | AAPL |
| 日期 | 2025-01-15 |
| 评级 | Overweight |
| 置信度 | medium |
| 市场 | US |

### A股特别说明

- 股票代码使用6位纯数字格式（如 600519、000858）
- `trad-skill` 内部根据 6 位代码前缀自动判断交易所（6 开头 → 上海 .SS；0/3 开头 → 深圳 .SZ），用户只需提供 6 位纯数字
- 数据源：默认走东方财富（Eastmoney）行情接口；可用 `trad-skill stock --source yahoo` 切换 Yahoo 通道（A股代码自动映射为 .SS/.SZ）
- 支持中文新闻和情绪分析
- 使用中国市场专用分析师提示词（`china_market_analyst.md`、`cn_news_analyst.md`）

### 港股特别说明

- 使用 4-5 位数字 + `.HK` 后缀（如 0700.HK 或 00700.HK；`trad-skill` 两种都接受，内部统一做 5 位零填充）
- 数据源：港股行情默认走东方财富接口（secid 116.<5位代码>）；`--source yahoo` 可切换 Yahoo 通道
- 支持港股通标的和港股主板股票

### A股基金特别说明

- 基金代码同为 6 位纯数字（如 000001），会与 A 股股票代码冲突（000001 是平安银行）。请使用 `fund` 子命令指定基金：`trad-skill fund --symbol 000001`
- ETF（510xxx/159xxx）可通过 `stock`（基于价格）和 `fund`（基于净值）两种方式分析
- 数据源：仅东方财富基金接口（无 Yahoo 回退）

---

## 架构

```
┌─────────────────────────────────────────────────────┐
│                   分析师团队（并行）                    │
│  ┌──────────┐ ┌──────────┐ ┌────────┐ ┌──────────┐ │
│  │ 市场分析师│ │情绪分析师│ │新闻分析师│ │基本面分析师│ │
│  └──────────┘ └──────────┘ └────────┘ └──────────┘ │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│            研究辩论（1-3 轮）                          │
│         看多研究员  ↔  看空研究员                      │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │     研究主管         │ → 投资计划
              └─────────────────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │      交易员          │ → 交易提案
              └─────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│            风控辩论（1-3 轮）                          │
│     激进派  ↔  保守派  ↔  中性派                      │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │   投资组合经理       │ → 最终决策
              └─────────────────────┘
```

### 配置参数

| 参数 | 范围 | 默认值 | 说明 |
|------|------|--------|------|
| `max_debate_rounds` | 1–3 | 1 | 研究辩论中看多/看空交锋轮数 |
| `max_risk_discuss_rounds` | 1–3 | 1 | 风控辩论中三方交锋轮数 |
| `output_language` | English / 中文 | 跟随用户语言 | 所有报告和最终决策的输出语言 |
| `market` | 自动检测 | — | 根据代码后缀自动识别市场 |

市场自动识别规则：

- 6 位纯数字（如 600519、000858）→ A股
- 4/5 位纯数字（如 0700、09988）→ 港股（无 `.HK` 后缀时）
- `.HK` 后缀 → 港股
- `-USD` 后缀 → 加密货币
- 其余 → 美股

---

## 项目结构

```
trad-skill/                        # 仓库根目录（元文件 + 安装器）
├── package.json                  # npm 入口（name: trad-skill）
├── bin/trad-skill.js             # 极简 JS launcher -> Rust 二进制（安装 + 数据）
├── README.md / README_CN.md       # 双语文档
├── CHANGELOG.md                   # 版本历史
├── AGENTS.md                      # AI 代理接入文档
├── LICENSE                        # Apache 2.0 许可证
├── .github/workflows/ci.yml      # CI: fmt + clippy + test + 7平台构建
├── crates/trad-data/              # Rust 源码（产出二进制名：trad-skill）
└── skills/
    └── tradingagents-analysis/    # 可安装的技能
        ├── SKILL.md               # 核心技能指令文件
        └── references/
            ├── prompts/               # 18个智能体角色提示词
            │   ├── market_analyst.md       # 市场分析师
            │   ├── sentiment_analyst.md    # 情绪分析师
            │   ├── news_analyst.md         # 新闻分析师
            │   ├── fundamentals_analyst.md # 基本面分析师
            │   ├── bull_researcher.md      # 看多研究员
            │   ├── bear_researcher.md      # 看空研究员
            │   ├── research_manager.md     # 研究主管
            │   ├── trader.md               # 交易员
            │   ├── aggressive_risk.md      # 激进风控
            │   ├── conservative_risk.md    # 保守风控
            │   ├── neutral_risk.md         # 中性风控
            │   ├── portfolio_manager.md    # 投资组合经理
            │   ├── china_market_analyst.md # 中国市场分析师
            │   ├── cn_news_analyst.md      # 中文新闻分析师
            │   ├── fund_market_analyst.md       # 基金市场分析师
            │   ├── fund_sentiment_analyst.md    # 基金情绪分析师
            │   ├── fund_news_analyst.md         # 基金新闻分析师
            │   ├── fund_fundamentals_analyst.md # 基金基本面分析师
            │   └── README.md               # 提示词索引
            ├── data-sources.md         # 数据源目录（美股+A股+港股）
            └── indicators.md           # 技术指标参考
```

---

## 数据源

### 美股数据源

| 数据源 | 提供内容 | API Key |
|--------|----------|---------|
| Yahoo Finance | 行情、基本面、新闻 | 免费 |
| Alpha Vantage | 行情、指标、基本面 | 免费额度 |
| FRED | 宏观经济指标 | 免费 |
| Polymarket | 预测市场概率 | 免费 |
| StockTwits | 散户情绪 | 免费 |
| Reddit | 社区讨论 | 免费 |

### A股/港股数据源

| 数据源 | 提供内容 | API Key |
|--------|----------|---------|
| Tushare | A股行情、基本面 | 需要 Token |
| AKShare | A股/港股行情、新闻、情绪 | 免费开源 |
| Baostock | A股历史数据 | 免费 |
| 通达信 TDX | 技术指标 | 免费 |

> 注意：Alpha Vantage、FRED、Polymarket、Tushare、Baostock 和 TDX 仅作为历史/出处参考列出；trad-skill 二进制直接使用 Yahoo Finance 与东方财富（Eastmoney）。详见 [references/data-sources.md](skills/tradingagents-analysis/references/data-sources.md) 了解实际接入的数据源与回退策略。

详见 [references/data-sources.md](skills/tradingagents-analysis/references/data-sources.md)。

---

## 数据工具（`trad-skill`）

数据通过 `trad-skill` Rust 二进制获取，提供行情数据（OHLCV + 指标）、新闻、基本面、情绪和基金数据，输出为紧凑格式，适合 LLM 提示词注入。平台二进制以五个 `@trad-skill/<platform>` npm 包的形式分发（作为 optional dependencies 安装），极简 launcher `bin/trad-skill.js` 在运行时解析出当前平台对应的二进制；同一个二进制同时承载安装和数据子命令。

> 代理必须用技能安装目录内的**绝对路径**来运行 `trad-skill`（如 `~/.agents/skills/tradingagents-analysis/bin/<platform>/trad-skill`），因为子代理的工作目录是用户项目，而非技能文件夹。`SKILL.md` 已指示主代理在派生子代理前先解析该路径。

```bash
# 行情数据：尾部 OHLCV + 预计算指标 + 可选统计
trad-skill stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats

# 或通过 bunx 直接运行（免安装）：
bunx trad-skill@latest stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats

# 或用绝对路径调用（代理实际调用方式）：
~/.agents/skills/tradingagents-analysis/bin/<platform>/trad-skill stock --symbol AAPL --start 2023-07-01 --end 2024-06-30 --tail 30 --stats

# A股：直接使用 6 位代码（自动路由到东方财富）
trad-skill stock --symbol 600519 --tail 30

# 显式指定数据渠道——例如所在地区无法访问 Yahoo Finance（报 "未知错误" / 403）时，
# 把美股改走东方财富通道：
trad-skill stock --symbol AAPL --source eastmoney

# 获取新闻（默认 --limit 8，摘要截断）
trad-skill news --symbol AAPL --days 7 --limit 8

# 获取基本面（精简关键指标表 + 公司概况）
trad-skill fundamentals --symbol AAPL

# 获取情绪数据（默认 --limit 15，--days 7 Reddit 时间窗）
trad-skill sentiment --symbol AAPL --limit 15 --days 7

# A股/港股 基本面/新闻自动走东方财富（无需 Yahoo）：
trad-skill fundamentals --symbol 600519
trad-skill news --symbol 600519
trad-skill stock --symbol 0700.HK        # 港股行情走东方财富
trad-skill news --symbol 0700.HK         # 港股新闻走东方财富
# 港股情绪暂不支持（StockTwits/Reddit 仅覆盖美股；东方财富千股千评未覆盖港股），
# 如有美股对应代码（如 BABA）可用该代码查询情绪。
```

| 子命令 | 默认值（紧凑） | 扩展参数 |
|---|---|---|
| `stock` | `--tail 30` + `--indicators` 开启 | `--stats`, `--raw`, `--source yahoo\|eastmoney` |
| `news` | `--limit 8`，200字符摘要 | `--limit N`, `--days N` |
| `fundamentals` | 精简关键指标表 | — |
| `sentiment` | `--limit 15`，`--days 7`（Reddit），8条消息/帖子 | `--limit N`, `--days N` |
| `fund` | 公募基金/ETF/LOF：净值历史 + 基金资料 + 重仓股 + 业绩表现（东方财富） | `trad-skill fund --symbol 000001 --tail 30` |

`stock --source` 用于选择数据渠道：默认按 symbol 自动识别——美股/加密货币走 Yahoo Finance，A股/港股走东方财富。传 `--source eastmoney` 可把美股改走东方财富（适用于 Yahoo 被区域封锁的场景）；传 `--source yahoo` 可强制走 Yahoo（A股/港股代码会映射为 `.SS`/`.SZ`/`.HK`）。东方财富不提供加密货币行情。

**Yahoo Finance 不可达时**（症状：数据中心/云 IP 上报 `未知错误`、`401 Unauthorized` 或 `403 Forbidden`）：A股的 `stock` / `fundamentals` / `news` 已自动走东方财富——直接传 6 位代码即可；美股行情加 `--source eastmoney`；美股 `fundamentals` / `news` 无东方财富对应源，代理会对这部分回退到网络搜索。

`trad-skill` 是**主要**数据源，优先尝试。它不是硬性依赖：当某个子命令执行失败或数据源不可用时，代理**仅对子命令未能提供的部分**回退到网络搜索/浏览器工具——绝不跳过二进制直接用网页搜索。

---

## 致谢

- **[TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents)** — 原创多智能体交易框架（Apache 2.0）
- **[hsliuping/TradingAgents-CN](https://github.com/hsliuping/TradingAgents-CN)** — 中国市场增强版
- **论文**: Xiao et al., "TradingAgents: Multi-Agents LLM Financial Trading Framework", [arXiv:2412.20138](https://arxiv.org/abs/2412.20138)

感谢上述项目的开源贡献，本技能的提示词和分析方法论均源自这些优秀项目。

---

## 免责声明

> ⚠️ **重要提示**：本技能**仅供研究与学习使用**。
>
> - 不构成任何投资建议、财务建议或交易建议
> - 过往表现不代表未来收益
> - 大语言模型分析具有非确定性，可能包含事实错误、虚构数据或有缺陷的推理
> - 投资决策前请务必咨询持牌金融顾问
> - 作者和贡献者不对基于本工具输出做出的任何交易决策承担责任
> - 投资有风险，入市需谨慎

---

## 许可证

[Apache License 2.0](LICENSE)
