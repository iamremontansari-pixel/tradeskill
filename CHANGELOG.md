# Changelog

All notable changes to this project will be documented in this file.

## [1.10.1] - 2026-08-11

### Fixed

- **HTTP 流式读取防 OOM（H1）**：`text_limited` / `json_limited` 弃用 `resp.bytes()` 全量读取，改为 `resp.chunk()` 流式累加并实时超限拒绝；新增公共 `bytes_limited()` 供 fund 复用。chunked 无 Content-Length 响应不再有内存耗尽风险（新增单元测试锁定）。
- **fund body 复用字节上限（H2）**：`fetch_fund_body` 改走 `http::bytes_limited()`，删除 fund.rs 自有的重复 `MAX_BODY_BYTES` 常量与无界读取。
- **installer 原子性（H3）**：bin 复制移到 swap 之前（先复制到 `tmp_dir/bin/<key>/`），bin 复制失败时 dest 不动、旧安装完整保留；backup 清理推迟到全部复制成功之后；新增 `test_install_bin_copy_failure_preserves_old` 与 `test_install_atomic_rollback`。
- **manual install curl 循环缺 4 个基金 prompt（H4）**：README/README_CN 手动安装脚本补全 `fund_market_analyst` / `fund_sentiment_analyst` / `fund_news_analyst` / `fund_fundamentals_analyst` 四个文件。
- **launcher install-token 过滤（M1）**：`bin/trad-skill.js` 丢弃 `install` token 时维护 `prevWasValueFlag` 状态（`--dir`/`--agent`/`--skills-dir`/`--bin-path` 后的 token 视为值不剥），`--agent claude install` 等形态不再重复路由 install。
- **skill 文档矛盾（M5-M9）**：china_market_analyst Tushare→TDX 引用修正；news_analyst 去掉 Yahoo 新闻过度声明；GBK 编码说明修正；Tushare 标注 not-wired（数据源优先级修正）；fund prompt 幻影变量删除。
- **Google News CDATA（M10）**：news 解析处理 `Event::CData`，description 不再被 `CDATA` 标签污染（测试锁定）。
- **org-participation 缩放（M17）**：Reddit org-participation 计数修正为按组织名聚合缩放，不再虚高。
- **残留目录清理（M18）**：install 启动时清理 `parent_dir` 下残留的 `.tradingagents-analysis.old-*` / `.tmp-*` 目录（最佳努力，失败不阻断）。
- **Rust 健壮性（L13-L21）**：fund `--days` clamp 到 `[1,3650]`；CSV 输出末行补 trailing newline；`strip_jsonp` 以 JSON 大括号（而非括号）锚定剥离；Yahoo crumb 刷新改 `tokio::sync::Mutex` 串行化（消除 TOCTOU 竞态）；fund profile `strip_tags` 输出解码 HTML entities（`&amp;` → `&`）；净值段标题显示取数窗 `days` 而非显示数 `tail`；标注东方财富 lsjz 接口 pageSize 上限 200 的截断说明；`rolling_std` 由平方和增量法改 Welford 在线算法（消除大数 catastrophic cancellation）。

### Changed (breaking)

- **`--limit` 上限收紧（L14）**：news/sentiment 的 `--limit` 现在 clamp 到 `[1,1000]`，`>1000` 直接退出码 2（此前仅被 50MB 响应体上限兜底）；fund `--days` clamp 到 `[1,3650]`。
- **CI build 并行化（L27）**：ci.yml build job 与 check job 并行（`check` 提前到 build 前），缩短 PR 反馈时间。

### CI

- **7-file version sync（M11）**：CI 新增 node 校验，断言 package.json / 5 个 npm manifest / Cargo.toml / 5 个 `@trad-skill/*` pin 与 package.json 版本一致。
- **Windows test job（M12）**：新增 windows-latest `cargo test` + smoke 测试。
- **musl 静态 pin + file 验证（M13）**：linux musl 目标显式 RUSTFLAGS 静态链接，并用 `file` 命令验证产物为 statically linked。
- **CI/Release matrix parity 断言（M14）**：断言两个 workflow 的 7 平台矩阵一致。
- **权限与超时（L26）**：workflow 权限收窄为只读 + `id-token: write` 仅 release 用；各 job 加 30 分钟 timeout。
- **actions SHA pin（L29）**：全部第三方 actions 改为 SHA pin（供应链安全）。
- **semver tag 触发（L34）**：release workflow 触发条件收窄为 `v*.*.*` 格式 tag。
- **scoped id-token（L35）**：id-token 权限从 workflow 级收到 npm-publish job 级。
- **release 校验强化（L31-L32）**：npm-publish 前校验 Cargo.lock 中 trad-data 版本与 tag 一致、CHANGELOG 含对应章节；`.gitignore` 覆盖已发布的 `npm/*/trad-skill[.exe]` 二进制。

### Notes

- **7-file version sync**：`package.json` `version` `1.10.0` -> `1.10.1`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）；`crates/trad-data/Cargo.toml` `version` -> `1.10.1`；5 个 `npm/<platform>/package.json` `version` -> `1.10.1`；`crates/trad-data/Cargo.lock` 同步至 `1.10.1`（`cargo build` 重新生成）。
- **3 项 intentional skips**（L22/L24/L25，详见审计报告 Out of scope）：L22 sentiment US 全源失败 Ok exit 0（设计选择）；L24 Intel Mac 不支持（AGENTS.md 已声明 7-target）；L25 npm `libc` 字段缺失（musl 静态二进制当前在 glibc 可跑，缺 libc 是正确表达）。

## [1.10.0] - 2026-08-08

### Added

- **`fund` 子命令**：A股基金（公募基金/ETF/LOF）分析数据工具，取数自东方财富基金 API：净值历史（lsjz，JSON + callback 包装）、基金资料（jbgk，HTML 表格：基金经理/规模/成立日期/管理人与托管人）、重仓股（jjcc，含最新价/涨跌幅）、业绩表现（jdzf，阶段涨幅与同类排名）。GBK/UTF-8 双解码回退（`encoding_rs`，纯 Rust 无 C 依赖），日频限流保护（≤1 req/s）。
- **4 个基金专属分析师 prompt**：`fund_market_analyst` / `fund_sentiment_analyst` / `fund_news_analyst` / `fund_fundamentals_analyst`（`skills/tradingagents-analysis/references/prompts/`），按 `china_market_analyst.md` 结构撰写，含内联「数据来源」段（`trad-skill fund` / 网页搜索兜底）。
- **SKILL.md §2/§4/§6/§8 基金分析文档**：§2 基金代码与股票代码冲突提示、§4 基金分析 agent 编排与 prompt 替换说明、§6 `trad-skill fund` 命令表行、§8 基金市场识别规则。
- **README/README_CN A股基金用法**：Usage 触发示例（「Analyze fund 000001（华夏成长）」/「Analyze ETF 510300（沪深300ETF）」）、Data Tool 命令表 `fund` 行、A股基金特别说明（6 位代码与股票代码冲突、ETF 双通道、仅东方财富基金 API）。
- **data-sources.md China Fund Sources**：4 个东方财富基金端点（含 Referer 要求、GBK 编码、限流）与 A 股基金数据降级说明。

### Notes

- `package.json` `version`: `1.9.3` -> `1.10.0`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.9.3` -> `1.10.0`。
- 5 个 `npm/<platform>/package.json` `version`: `1.9.3` -> `1.10.0`。
- `crates/trad-data/Cargo.lock` 同步至 `1.10.0`（`cargo build` 重新生成）。
- **偏差（deviation）**：基金专属 prompt（T5）为**新撰写**而非逐字提取——本机无 TradingAgents-CN fork 可作逐字来源。这偏离了 AGENTS.md 的「When updating prompts, re-extract verbatim from source — never rewrite from memory」规则；已按 `china_market_analyst.md` 既有结构撰写，并在 T7 的 prompts/README.md 中标注 Source Repo: "Authored new"。

## [1.9.3] - 2026-08-04

### Fixed

- **响应体大小限制全覆盖**：1.9.2 引入的 50 MB `text_limited()` 限制此前未覆盖全部读取点；本次补齐所有剩余无界读取：sentiment（StockTwits/Reddit/东方财富四处）与 fundamentals（东方财富两处）改走新增的 `json_limited()`（大小限制 + JSON 解析）；Yahoo crumb、Yahoo 直连探测、重试层错误体、东方财富新闻 JSONP 改走 `text_limited()`。新增 `check_content_length` Content-Length 预检（超限立即拒绝，避免下载）并附单元测试。

### Changed

- **发布流程**：`workflow_dispatch` 手动发布时 check/build/release 三个 job 改为统一检出解析后的 tag（此前仅 npm-publish 检出 tag，tag 不在默认分支顶端时二进制与清单可能漂移）；npm 发布前新增校验 `package.json` 的 5 个 `@trad-skill/*` optionalDependencies pin 与 tag 一致且数量恰为 5；`fail_on_unmatched_files` 改为 true，并在创建 GitHub Release 前断言 7 个平台二进制全部存在。
- **指标快照输出头**：「技术指标快照（脚本预计算）」→「技术指标快照（预计算）」（去除 Python 时代措辞；测试断言同步锁定新标题）。
- **文档同步**：README/README_CN 修正 A股/港股陈旧的「AKShare → yfinance」数据源描述（实际默认东方财富、可 `--source yahoo` 切换）、补齐 4/5 位纯数字港股代码的市场识别规则、修正二进制分发描述（五个 `@trad-skill/*` 平台包 + 启动器解析）并中英对齐（摘要表、A股/港股说明、TDX 行）；AGENTS.md 结构树补齐 npm/、CONTRIBUTING.md、CLAUDE.md、release.yml，修正 launcher 描述与 clippy 命令；CONTRIBUTING.md 与 install.rs 注释去除 strategy-1 残留；SKILL.md 市场列表补 A股、补港股情绪不支持说明与 `stock` 默认日期窗（end=今天、start=今天−365 天），data-sources.md 去除 "script" 残留措辞。

### Notes

- `package.json` `version`: `1.9.2` -> `1.9.3`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.9.2` -> `1.9.3`。
- 5 个 `npm/<platform>/package.json` `version`: `1.9.2` -> `1.9.3`。
- `crates/trad-data/Cargo.lock` 重新同步至 `1.9.3`（该文件自 v1.9.2 起停留在 `1.9.1`）。

## [1.9.2] - 2026-08-02

### Fixed

- **北交所 920xxx 路由错误**：`cn_market_id` 改用 `starts_with("900")` 精确匹配沪B，北交所 920xxx 代码正确路由到市场 0（此前 `starts_with('9')` 误判为沪市）。
- **港股 symbol 注入防护**：`hk_eastmoney_code` 添加纯数字校验，非数字输入返回安全占位码（防止 `&`/`?` 注入东方财富 URL 查询参数）。
- **安装器健壮性**：过滤空 `USERPROFILE`/`HOME` 环境变量（防止 CI/容器下静默安装到 cwd 相对路径）；`--skills-dir` 改用 `expand_tilde`（与 `--dir` 一致）；SKILL.md 标记检查改用 `.is_file()`；rename 失败时清理临时目录并报告备份路径；`--no-bin` 时不再创建空 `bin/` 目录。
- **CLI 参数互斥**：顶层 `--dir`/`--agent` 添加 `conflicts_with`，在 clap 解析层即报错（此前仅运行时检查）。
- **HTTP 健壮性**：新增 `text_limited()` 响应体大小限制（50 MB），防止上游异常导致 OOM；指数退避 `2u64.pow(attempt.min(6))` 防止溢出 panic。
- **Yahoo crumb 缓存 TTL**：添加 1 小时过期机制，过期后主动刷新（此前缓存永不过期，crumb 轮换后可能持续失败）。
- **CN/HK 基本面退出码**：全部数据源失败时返回 Err（exit 1），与美股行为一致（此前返回 Ok + exit 0）。
- **JS launcher**：仅剥离首个位置参数 `install`（此前 `filter(a !== 'install')` 误删 `--dir install` 等合法值）。

## [1.9.1] - 2026-08-02

### Fixed

- **安装器数据安全**：目标目录已存在但缺 `SKILL.md`（非本技能安装）时拒绝覆盖，不再无条件 `remove_dir_all` 误删用户目录；`--skills-dir` 指向已安装副本（源==目标）时直接报错；复制改为「临时目录 + 换名就位」，中途失败不破坏上一次可用安装。
- **安装目标路径**：Windows 下优先 `USERPROFILE`（Git Bash/CI 的 `HOME` 是 POSIX 路径，会被解析到错误的盘符根目录）；平台二进制复制失败且未指定 `--no-bin` 时以非零码退出，不再打印"已安装"后退出 0。
- **`bunx trad-skill@latest install`**：显式 `install` 子命令此前被透传、回落到构建机路径而必然失败；现在 launcher 同样注入 `--skills-dir`（用户自带的后者胜）。`--version` 经 launcher 可用。
- **CLI 参数校验**：`--start`/`--end` 统一按 `YYYY-MM-DD` 校验（格式错误、`end < start`、未来日期 → 明确报错，退出码 2）；`--tail` 收敛到 [1, 500]；`--limit 0` 报错。
- **退出码**：参数错误=2、取数/网络失败=1（此前全部为 1，`--raw` 空数据甚至退出 0）；`fundamentals`/`news`/`sentiment` 改由 `Result` 边界区分错误，不再依赖 `错误:` 字符串前缀探测。
- **A股成交量单位**：东方财富 A 股渠道的成交量单位是手（1 手=100 股），已统一换算为股，与 Yahoo 渠道一致（此前系统性小 100 倍）。
- **市场识别**：4 位纯数字（如 `0700`）正确识别为港股（此前误判美股）；`900xxx`（沪B）正确路由到沪市；symbol 入口统一 trim，尾随空白不再导致请求失败。
- **Yahoo crumb 握手**：`Invalid symbol` 等确定性错误不再白白做两次握手请求；HTTP 200 但响应体带 `finance`/`quoteSummary` 错误的形态现在也会触发 crumb 重试；crumb 进程级缓存（批量取数只握手一次），带 crumb 请求失败自动失效重取。
- **东方财富美股通道**：校验响应 `market` 字段与请求交易所前缀一致，避免同名代码取到另一家公司的数据。
- **其他**：URL 统一百分号编码（symbol 含 `&`/`?` 不再注入查询参数）；Reddit 源改走共享重试 + 浏览器 UA；`fmt_val`/`fmt_num` 不再输出 `inf`/`-0.00`；RSI 持平市场返回中性 50；空数据报告不再以 0 退出。

### Changed

- 依赖裁剪：`tokio` 由 `full` 收敛为 `rt`/`rt-multi-thread`/`macros`/`time`；`serde`/`chrono` 去掉未用 feature；release 增加 `panic = "abort"`。
- HTTP 默认 User-Agent 由 `CARGO_PKG_VERSION` 生成，不再硬编码版本号；429 响应优先遵循 `Retry-After`（上限 30s）。
- 东方财富 kline 抓取骨架提取为 `fetch_eastmoney_kline` 公共函数，消除 cn/hk/us_em 三处约 90 行重复。
- 技能文档：`indicators.md`/`prompts/README.md`/`data-sources.md` 中已删除 Python 脚本的引用全部改写为 `trad-skill` 子命令；市场自动检测规则在 SKILL.md §2/§8 与代码保持一致。
- 发布流程：npm-publish 按解析出的 tag checkout，并在发布前校验 6 个 npm manifest + Cargo.toml 与 tag 版本一致；changelog 提取改为字面精确匹配；`aarch64-pc-windows-msvc` 构建固定 `windows-2025` 镜像。

## [1.9.0] - 2026-08-02

### Added

- **`sentiment` 新增 `--days`（默认 7）**：控制 Reddit 帖子时间窗（≤7 天按 `week`、>7 天按 `month`）。此前 Reddit 时间窗硬编码 7 天，`--limit` 仅影响 StockTwits。同步更新 `SKILL.md` 与 `README`/`README_CN`。

### Fixed

- **Yahoo `stock` 行情漏掉 `--end` 当天数据**：Yahoo v8 chart 的 `period2` 是排他上界（`timestamp < period2`），而日线时间戳为当天 00:00 UTC，直接用 `end` 当天 00:00 作 `period2` 会漏掉 `end` 当天 K 线（默认 `--end today` 则丢当天）。`period2` 加 1 天让端点包含在内，与东方财富通道（含端点）行为一致。
- **`get_with_retry` 对不可恢复 4xx 重试浪费数秒**：此前对 400/401/403/404 等也按指数退避重试——Yahoo 401/403 需 crumb、404 无效代码，重试必失败却白等 1–7s。新增 `is_retryable_status`：仅 5xx 与 429 限流重试，4xx 立即失败。新增 `test_is_retryable_status`。
- **RSI 平价市场返回 100 应为 50**：`avg_loss==0` 时一律返回 100，但价格连续持平（`avg_gain==avg_loss==0`）应为中性 50，与同文件 MFI 的边缘处理一致。新增 `test_rsi_flat_market_is_neutral`。
- **东方财富 klines 脏字段回落 0.0 污染序列**：`parse_eastmoney_klines` 对 open/close/high/low 解析失败原回落 `0.0`，一根 `close=0` 的 K 线会让收益率/波动率/均线失真。改为关键字段解析失败即跳过该行（成交量仍允许 0）。新增 `test_parse_eastmoney_klines_skip_bad_ohlc`。
- **Yahoo 解析器对 null 时间戳生成 1970-01-01 假行**：`ts_val` 非 i64 时 `unwrap_or(0)` 产生 1970 行，改为跳过。新增 `test_parse_yahoo_skips_null_timestamp`。
- **新闻摘要残留 HTML 实体**：`strip_html` 仅删 `<...>` 标签，`&amp;`/`&lt;`/`&quot;`/`&#39;`/`&nbsp;` 原样残留。新增 `decode_html_entities`（命名实体 + 数字实体，未知保留），在剥标签之后调用（先解码会把 `&lt;`/`&gt;` 还原成尖括号被误删）。扩展 `test_strip_html`。

### Changed

- 移除死字段 `ReportOptions.raw`（恒为 false：`--raw` 走 `ohlcv_to_csv` 直连分支，从不经过 `build_compact_report`）及其死分支与 `test_raw_mode`。
- `bin/trad-skill.js` 移除未使用的 `catch (e)` 形参。

### Notes

- `package.json` `version`: `1.8.6` -> `1.9.0`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.6` -> `1.9.0`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.6` -> `1.9.0`。

## [1.8.6] - 2026-08-02

### Fixed

- **东方财富/A股 markdown 表格分隔行多一个尾随 `|`**：`build_cn_financial_table`、`build_us_timeseries_table`（fundamentals.rs）与 `format_cn_comment_table`（sentiment.rs）用 `format!("|{}|", "---|".repeat(N))` 生成分隔行，但 `"---|".repeat(N)` 末尾已带 `|`，外层再补一个 `|` 即产生形如 `|---|---|---|---|---|---||` 的尾随 `||`，使分隔行列数比表头/数据行多一列，渲染为多出一空列的错位表格（影响所有 `fundamentals`/`sentiment` 返回的表格）。改为 `format!("|{}", "---|".repeat(N))`（仅前导 `|`），三处分隔行管道符数与表头一致。新增「分隔行管道符数 == 表头」断言防回归。
- **A股/港股 `news` 的 `--days` 被忽略**：`fetch_eastmoney_news` 接受 `days` 但仅在 Google News 降级分支使用，东方财富主路径 `sort=default` 且无日期过滤，`--days 7` 常返回数周前旧闻（实测 `0700.HK` 返回 7 月 2-9 日回购公告，而当日最近 7 天应为 7 月 26 日-8 月 2 日）。现 `sort=time` 按发布时间倒序拉取，新增 `parse_eastmoney_article_date` 解析文章 `date` 字段（如 `2026-07-30 21:25:00`）并按 `days` 客户端过滤，候选池 `pageSize=max(limit,30)` 过滤后取 `limit`；全部被日期过滤或无结果时仍降级 Google News（其自身按 `when:days` 过滤）。新增 5 个单元测试覆盖日期解析/过滤/limit/空数组。

### Changed

- 东方财富新闻表头改为「最近 N 天，共 M 条」（与 Yahoo/Google 新闻一致，体现 days 过滤已生效）。
- HTTP user-agent 更新为 `trad-skill/1.8.6`。

### Notes

- `package.json` `version`: `1.8.5` -> `1.8.6`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.5` -> `1.8.6`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.5` -> `1.8.6`。

## [1.8.5] - 2026-08-01

### Fixed

- **港股 `stock`/`fundamentals` 对 `.HK` 代码失效**：东方财富港股端点（push2his 行情、push2 基本信息、datacenter 财务）要求 5 位零填充 secid（如 `116.00700`）。`market/hk.rs` 此前直接拼 `secid=116.{symbol}`（`0700.HK` -> `116.0700.HK`），`fundamentals.rs` 虽去 `.HK` 但未补零（`0700.HK` -> `116.0700`），二者均返回 `data:null`，导致文档主用例 `0700.HK`/`9988.HK` 静默失败（仅 5 位无后缀形式 `09988` 可用）。新增共享 `market::hk_eastmoney_code()`（去 `.HK` + 补零到 5 位），`hk.rs`（secid）与 `fundamentals.rs`（secid + datacenter_code）统一使用。
- **港股 `news` 落到美股分支**：`fetch_news` 此前仅特判 A股，港股走 Yahoo + Google（美股导向）。现 `Market::HKStock` 走东方财富文章搜索（5 位代码），返回相关港股新闻（实测 `00700` 命中数百条）。
- **港股 `sentiment` 发起无效请求**：StockTwits/Reddit 仅覆盖美股、东方财富千股千评/机构参与度未覆盖港股（实测均返回「返回数据为空」），原代码仍并行请求并输出两段「数据源不可用」。现港股直接返回明确「暂不支持」提示，避免 4 次无效请求。
- **`http` 重试丢弃错误响应体**：非成功响应未读取即 drop，底层连接无法复用。现读取并丢弃响应体后再重试，连接归还连接池。
- **install 测试并行竞态**：`agent_dir_mapping`/`default_target_is_agents`/`expand_tilde_basic` 写进程级 `HOME`/`USERPROFILE`，cargo 并行测试时互相覆盖偶发失败。以 `std::sync::Mutex` 串行化这三个测试。

### Changed

- **港股 OHLCV URL 传 `beg`/`end`**：原 `end=20500000&lmt=1000000` 拉全量历史再客户端过滤，现服务端按日期过滤（保留客户端过滤兜底）以减少传输；并切换到规范主机 `push2his.eastmoney.com`（与其他东方财富模块一致）。
- **`url_encode` 去重**：`yahoo.rs::url_encode` 与 `news.rs::urlencoding_encode` 逐字节相同，合并为 `http::url_encode` 单一实现。
- **`fetch_news` 去除冗余 clone**：重构双源失败分支，避免 `yf_news.clone()`。
- **`fmt_val` 注释**：说明显式 `(v*10000).round()` 强制四舍五入（远离零），与 `{:.4}` 的银行家舍入在末位 5 时不同，勿简化。
- HTTP user-agent 更新为 `trad-skill/1.8.5`。

### Notes

- `package.json` `version`: `1.8.4` -> `1.8.5`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.4` -> `1.8.5`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.4` -> `1.8.5`。

## [1.8.4] - 2026-08-01

### Fixed

- **港股 `fundamentals` Yahoo 403**：`fetch_fundamentals` 此前将 `Market::HKStock` 路由到 Yahoo（数据中心/云 IP 常返回 403）。现提取共享 `fetch_eastmoney_fundamentals` helper（`EastmoneyParams` struct），新增 `fetch_hk_fundamentals`（secid `116.{code}`），港股基本面自动走东方财富，与 A股行为一致；若 datacenter 无港股财务指标行，财务表优雅降级为「暂不可用」，个股基本信息仍正常输出。

### Changed

- HTTP user-agent 更新为 `trad-skill/1.8.4`。

### Notes

- `package.json` `version`: `1.8.3` → `1.8.4`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.3` → `1.8.4`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.3` → `1.8.4`。

## [1.8.3] - 2026-08-01

### Fixed

- **美股 `fundamentals` 公司概况打印原始 JSON**：`fmt_num` 新增对 Yahoo `{fmt,raw}` 值对象的处理（优先取 `fmt`，如 "466.82B"/"148.75%"），修复 ROE/总营收/利润率等显示为 `{"fmt":...,"raw":...}` 的问题。
- **美股 `fundamentals` 市值/PE/PB/公司名 N/A**：quoteSummary 追加 `price`/`summaryDetail` 模块，并按 `price.marketCap`→`defaultKeyStats.marketCap`、`summaryDetail.trailingPE`→`defaultKeyStats.trailingPE`、`defaultKeyStats.priceToBook`→`summaryDetail.priceToBook`、`longName`→`shortName`→symbol 依次兜底。
- **美股 `fundamentals` 财务报表大面积 N/A**：Yahoo quoteSummary 的 `incomeStatementHistory`/`balanceSheetHistory`/`cashflowStatementHistory` 模块已常返回空数组，改用 yfinance 同款的 **fundamentals-timeseries** 接口按年度取数（营收/净利/摊薄EPS/毛利/总资产/总负债/股东权益/经营现金流/自由现金流，金额用 `reportedValue.fmt` 显示）。
- **A股 `sentiment` 个股评论 综合得分/目前排名/关注指数 全 N/A**：`format_cn_comment_table` 字段名修正为东方财富 `RPT_DMSK_TS_STOCKNEW` 的实际字段 `TOTALSCORE`/`RANK`/`FOCUS`（原 `COMMENT_SCORE`/`CURRENT_RANK`/`FOCUS_INDEX` 不存在）；`RANK` 按整数显示。

### Changed

- HTTP user-agent 更新为 `trad-skill/1.8.3`。

### Notes

- `package.json` `version`: `1.8.2` → `1.8.3`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.2` → `1.8.3`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.2` → `1.8.3`。

## [1.8.2] - 2026-08-01

### Fixed

- **A股基本面「关键财务指标」表 7/8 指标为 N/A**：`build_cn_financial_table` 使用的东方财富字段名（`BASIC_EPS` / `BASIC_BPS` / `WEIGHTAVG_ROE` / `TOTAL_OPERATE_INCOME` / `PARENT_NETPROFIT` / `XSJLR` / `OPERATE_CASHFLOW`）在 `RPT_F10_FINANCE_MAINFINADATA` 中并不存在。改为实际字段：`EPSJB` / `BPS` / `ROEJQ` / `TOTALOPERATEREVE` / `PARENTNETPROFIT` / `XSMLL`(毛利率) / `XSJLL`(净利率) / `MGJYXJJE`。
- **毛利率/净利率标反**：旧代码把 `XSJLL`（销售净利率）标成「毛利率」。现已区分 `XSMLL`=毛利率、`XSJLL`=净利率。

### Changed

- **A股基本面精表扩充至 14 项**：新增 每股经营现金流、ROIC、毛利、扣非净利润、营收同比、净利同比、资产负债率。
- **金额按亿/万换算**：新增 `fmt_cn_amount`，营业总收入/毛利/净利润/扣非净利润等金额显示为「X.XX亿」（如 547.03亿），更易读。
- HTTP user-agent 更新为 `trad-skill/1.8.2`。

### Notes

- `package.json` `version`: `1.8.1` → `1.8.2`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.1` → `1.8.2`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.1` → `1.8.2`。

## [1.8.1] - 2026-08-01

### Fixed

- **`fundamentals` / `news` 401 Unauthorized**: Yahoo Finance 的 `v10/finance/quoteSummary` 端点（`fundamentals.rs`、`news.rs`）与 chart 一样需要 crumb token，此前直连导致 `错误: 获取 AAPL 基本面数据失败 - HTTP 请求失败: 401 Unauthorized`。现复用与 `stock` 相同的 cookie + crumb + 浏览器 UA 握手。
- 抽取共享模块 `yahoo.rs`（`BROWSER_UA` / `url_encode` / `get_crumb` / `append_crumb` / `yahoo_get_body`），`market/us.rs`、`fundamentals.rs`、`news.rs` 统一复用，消除重复的握手逻辑。
- Yahoo 不可达时 `fundamentals` 错误信息附带指引（A股用 6 位代码自动走东方财富）。

### Changed

- 文档（`README.md` / `README_CN.md` / `SKILL.md` / `references/data-sources.md`）：新增 A股 `fundamentals` / `news` 示例（自动走东方财富），并强化指引——A股优先东方财富源；Yahoo 不可达（`未知错误` / `401` / `403`）时美股行情用 `--source eastmoney`，美股 fundamentals/news 回退网络搜索。
- HTTP user-agent 更新为 `trad-skill/1.8.1`。

### Notes

- `package.json` `version`: `1.8.0` → `1.8.1`（+ 5 个 `optionalDependencies` `@trad-skill/*` pin）。
- `crates/trad-data/Cargo.toml` `version`: `1.8.0` → `1.8.1`。
- 5 个 `npm/<platform>/package.json` `version`: `1.8.0` → `1.8.1`。

## [1.8.0] - 2026-08-01

### Added

- **`stock --source yahoo|eastmoney` channel flag**: override the auto-detected data channel. `--source eastmoney` routes US stocks through a new Eastmoney push2his channel (tries secid `105`/`106`/`107` = NASDAQ/NYSE/AMEX) — the workaround for regions where Yahoo Finance is blocked. `--source yahoo` forces Yahoo (A-share/HK symbols are mapped to `.SS`/`.SZ`/`.HK`). New module `market/us_em.rs`; routing helpers in `market/mod.rs`.
- **A-share + channel examples in docs**: `README.md` / `README_CN.md` data-tool sections and `SKILL.md §6` now show `stock --symbol 600519` and `stock --symbol AAPL --source eastmoney`; `references/data-sources.md` documents the channel-selection matrix.
- **AGENTS.md "Git workflow" section**: codifies branch-first, batched Conventional Commits, keeping AGENTS/README/CHANGELOG in sync, gating before push, and opening a PR for user review.
- **Tests**: offline unit tests for `parse_yahoo_response` error paths (incl. the former "未知错误" case), crumb URL-encoding, and the symbol→Yahoo / US-Eastmoney secid routing helpers; plus `#[ignore]`d live network tests for the Yahoo and Eastmoney AAPL paths (`cargo test -- --ignored`).

### Fixed

- **Yahoo Finance "未知错误" from datacenter/cloud IPs** (e.g. the reported Korean Alibaba Cloud server): `market/us.rs` now performs the yfinance cookie + **crumb** handshake (new `get_crumb`), sends a realistic **browser User-Agent** on every Yahoo request, and uses the `query2` endpoint. A direct request is tried first; an empty / `chart.error=null` response (the exact "未知错误" symptom) now triggers the crumb retry instead of surfacing a bare error.
- **Clearer Yahoo errors**: `parse_yahoo_response` reports the Yahoo error `code` / `description` when present instead of "未知错误", and the empty-result / final-failure messages point users to `--source eastmoney`.
- reqwest gains the `cookies` feature and the shared client enables `cookie_store` (required for the crumb handshake). Pure-Rust deps only; all 7 build targets unaffected.

### Changed

- `market::fetch_ohlcv` takes an extra `source: Option<Source>` parameter; `OhlcvRow` derives `Debug, Clone, PartialEq`.
- `http::get_with_retry` delegates to a new header-aware `get_with_retry_headers` (so Yahoo can override the UA while keeping retry/backoff).
- HTTP user-agent updated to `trad-skill/1.8.0`.

### Notes

- `package.json` `version`: `1.7.0` → `1.8.0` (+ its 5 `optionalDependencies` `@trad-skill/*` pins).
- `crates/trad-data/Cargo.toml` `version`: `1.7.0` → `1.8.0`.
- All 5 `npm/<platform>/package.json` `version`: `1.7.0` → `1.8.0`.

## [1.7.0] - 2026-08-01

### Added

- **Unified Rust binary `trad-skill`**: the installer and data tool now ship as a single Rust binary. With no subcommand it runs the installer; `stock` / `news` / `fundamentals` / `sentiment` are data subcommands; an explicit `install` subcommand is also accepted. All primary logic (CLI parsing, install, data fetching) lives in Rust.
- **Single `bunx trad-skill@latest` entry point**: `bunx trad-skill@latest` installs the skill; `bunx trad-skill@latest stock --symbol AAPL` fetches data without an install. The previous separate `trad-data` launcher is removed.

### Changed

- **All install commands in docs use `@latest`** to avoid stale caches in bunx/npx.
- **Binary renamed** from `trad-data` to `trad-skill` (crate name `trad-data` in `crates/trad-data/` is unchanged to avoid path churn). The npm `bin` map now exposes only `trad-skill`; the previous `trad-data` bin entry is removed.
- **`bin/trad-skill.js` is the only Node launcher** and handles both install and data dispatch. When the first user arg is a data subcommand (`stock`/`news`/`fundamentals`/`sentiment`) or `install`, args are passed through verbatim; otherwise the launcher prepends `install --skills-dir <pkgRoot>/skills/tradingagents-analysis`. The previous `bin/trad-data-wrapper.js` is deleted.
- Install flags (`--agent`, `--dir`, `--skills-dir`, `--bin-path`, `--no-bin`, `--dry-run`) are accepted as **top-level flags** on the Rust binary in addition to `trad-skill install <flags>`, so `bunx trad-skill@latest --agent claude` resolves to install mode directly.
- Platform npm packages now ship `trad-skill` / `trad-skill.exe` instead of `trad-data` / `trad-data.exe`; CI artifact names and release filenames updated accordingly.
- HTTP user-agent updated to `trad-skill/1.7.0`.
- `package.json` `description` refreshed to reflect the unified installer + data CLI.
- Docs (`README.md`, `README_CN.md`, `SKILL.md`, `AGENTS.md`, `CONTRIBUTING.md`, `.github/PULL_REQUEST_TEMPLATE.md`, `references/data-sources.md`) updated for the unified command and `@latest` pinning.

### Removed

- `bin/trad-data-wrapper.js` (merged into `bin/trad-skill.js`).
- The `trad-data` bin entry in `package.json` (use `trad-skill <subcommand>` instead).

### Notes

- `package.json` `version`: `1.6.0` → `1.7.0`.
- `crates/trad-data/Cargo.toml` `version`: `1.6.0` → `1.7.0` (added `[[bin]] name = "trad-skill"`).
- All 5 `npm/<platform>/package.json` `version`: `1.6.0` → `1.7.0` with `files` switched to the `trad-skill[.exe]` binary.

## [1.6.0] - 2026-07-31

### Added

- **Rust installer**: skill installation is now handled by the `trad-data install` subcommand (Rust, `crates/trad-data/src/install.rs`) instead of the Node `install.mjs`. The npm `trad-skill` bin entry is a thin JS launcher (`bin/trad-skill.js`) that resolves the platform binary and execs the Rust installer.
- **`bunx` support**: `bunx trad-skill` installs the skill (or `npx trad-skill`, identical); `bunx trad-data <subcommand>` runs the data tool directly with no install.
- **`--dry-run`** flag: print the install plan without writing anything.
- **CONTRIBUTING.md** and a GitHub PR template: branching model (GitHub Flow), Conventional Commits, PR checklist, and the explicit 7-file release version-bump procedure.

### Changed

- **Default install target is now `~/.agents/skills`** (generic agent directory) instead of `~/.claude/skills`. Use `--agent claude` / `--agent opencode` / `--dir <path>` to override.
- **Installer architecture**: installer logic moved from `install.mjs` (Node ESM) into the Rust binary; the platform binary is self-copied via `std::env::current_exe()`.
- `npx skills add ...` (vercel-labs/skills CLI) is **deprecated** in the docs in favor of `bunx trad-skill`.

### Fixed

- **Doc examples**: `trad-data market ...` → `trad-data stock ...` (the clap subcommand is `stock`, not `market`) in both READMEs.
- Structure tree in READMEs: `install.mjs` → `bin/trad-skill.js`; CI platform count 6 → 7.

## [1.5.4] - 2026-07-30

### Fixed

- **npm publish ENEEDAUTH in CI**: restored `registry-url` in `actions/setup-node` (required for OIDC Trusted Publishing to locate the registry); added post-setup step to strip deprecated `always-auth` line from generated `.npmrc`.
- **npm publish warnings resolved**: `repository` field changed from shorthand string to object form in all 6 package.json files.
- **GitHub Release creation on workflow_dispatch**: added explicit `tag_name` to `softprops/action-gh-release` so releases work with both tag push and manual dispatch.
- **Platform package re-publish tolerance**: publish step now skips already-published versions with a warning instead of failing the entire job.

### Changed

- **npm Trusted Publishing (OIDC)**: release workflow publishes via `--provenance` with GitHub Actions OIDC — `NPM_TOKEN` secret no longer required.
- `package.json` `version`: `1.5.1` → `1.5.4`.
- `crates/trad-data/Cargo.toml` `version`: `1.5.1` → `1.5.4`.

## [1.5.1] - 2026-07-29

### Added

- **Platform binary delivery via npm optionalDependencies**: 5 platform-specific packages (`@trad-skill/{win32-x64,win32-arm64,darwin-arm64,linux-x64,linux-arm64}`) published alongside the main package. Users now download only their platform's ~4MB binary instead of all 7 (~28MB). Follows the esbuild/@swc/core distribution pattern.

### Fixed

- **Install commands updated**: `npx halfoffive/trad-skill` → `npx trad-skill` in both READMEs (package published on npmjs.com).
- **Installer binary resolution**: no longer reports "binary not found" — resolves from `@trad-skill/<platform>` optionalDependency package via `createRequire`.

### Changed

- **npm publishing**: release workflow now publishes 5 platform packages + main package (scoped `@trad-skill/*`, `--access public`). NPM_TOKEN retained temporarily; OIDC migration deferred.
- **GitHub Actions upgraded to Node 24 runtime**: `actions/checkout` v5→v6, `actions/setup-node` v4→v5, `softprops/action-gh-release` v2→v3.
- `package.json` `version`: `1.5.0` → `1.5.1`.
- `crates/trad-data/Cargo.toml` `version`: `1.5.0` → `1.5.1`.

## [1.5.0] - 2026-07-28

### Changed

- **Shared HTTP client**: `reqwest::Client` created once in `main()` and passed to all subcommands, enabling connection pooling across requests.
- **Parallel data fetching**: Yahoo Finance + Google News fetches parallelized via `tokio::join!`; StockTwits + Reddit parallelized; 3 subreddit fetches parallelized.
- **Release profile optimization**: `strip = true`, `lto = true`, `codegen-units = 1` — release binary reduced from 4.8 MB to 3.9 MB (-18%).
- **ReportOptions struct**: replaced 8-parameter `build_compact_report` with a config struct, removed `#[allow(clippy::too_many_arguments)]`.
- **Shared kline parser**: extracted `parse_eastmoney_klines()` to `market/mod.rs`, eliminating duplicate parsing logic between `cn.rs` and `hk.rs`.
- `package.json` `version`: `1.4.0` → `1.5.0`.
- `crates/trad-data/Cargo.toml` `version`: `1.4.0` → `1.5.0`.

### Fixed

- **Removed unused `csv` dependency**: declared in `Cargo.toml` but never imported.
- **`us.rs` `.unwrap()` violation**: `date_to_unix` used `.unwrap()` on `and_hms_opt`; replaced with `.ok_or_else()` (AGENTS.md compliance).
- **`rolling_std` usize underflow**: `i - period + 1` panicked in debug mode when `i == period - 1`; changed to `i + 1 - period`.
- **HK stock date filter format mismatch**: `parse_eastmoney_klines` compared `YYYY-MM-DD` dates against `YYYYMMDD` filter, silently filtering out all rows; now normalizes format before comparison.

### Added

- **24 new unit tests** (28 total, was 4): coverage for `indicators` (SMA/EMA/RSI/Bollinger/ATR edge cases), `format` (empty data, raw mode, CSV, tail truncation), `parse_eastmoney_klines` (basic, date filter, short rows, empty), `news` utilities (`strip_html`, `strip_jsonp`, `urlencoding_encode`, `truncate`).
- **Release workflow quality gate**: `release.yml` now runs `cargo fmt --check` + `clippy -D warnings` + `cargo test` before building release binaries.
- **SKILL.md §8**: documented 5-digit pure number → HK stock auto-detection rule.

## [1.4.0] - 2026-07-27

### Added

- **npm auto-publish**: release workflow now publishes to npm automatically on tag push (requires `NPM_TOKEN` secret in GitHub Actions).
- **npm provenance**: published packages include provenance attestation for supply-chain transparency.

### Fixed

- **README.md restored**: accidentally deleted in commit 116c687 ("屏蔽trae ide文件"); restored from git history.
- **release.yml workflow_dispatch**: `inputs.tag` was defined but never used; manual dispatch now correctly resolves the tag for changelog extraction.
- **Artifact action versions**: release.yml unified from v4 to v7 (matching ci.yml).
- **Version sync**: Cargo.toml and http.rs user-agent bumped from 0.1.0 to 1.4.0 (matching package.json).
- **Removed no-op prepublishOnly**: the echo-only script added no value for automated publishing.

### Changed

- `package.json` `version`: `1.3.6` → `1.4.0`.
- `crates/trad-data/Cargo.toml` `version`: `0.1.0` → `1.4.0`.

## [1.3.6] - 2026-07-26

### Fixed (round 6)

30 confirmed bugs fixed (4 HIGH / 8 MEDIUM / 18 LOW). Review method: 3 parallel general-purpose sub-agents (Python scripts / Prompts+SKILL / Docs+Installer), each read-only, with cross-validation against source repos `../TradingAgents` and `../TradingAgents-CN`. 6 of the 30 are documented as known limitations (verbatim constraints or low-risk edge cases where code change is risky/out-of-scope): R6-5, R6-11, R6-19, R6-22, R6-23, R6-24.

**HIGH (4)**

- **R6-1 `fetch_us_fundamentals` ticker 作用域 bug**：`ticker = yf.Ticker(symbol)` 在 try 块内，若抛异常（网络瞬断/rate limit）则 `ticker` 永不赋值，后续三大报表 try 块引用 `ticker.financials` 抛 `NameError` 被各自 except 捕获后置 None，最终输出 "无数据"，掩盖公司概况网络失败。预声明 `ticker = None` + 三大报表前 `if ticker is None` 短路。
- **R6-2 `fetch_yfinance_news` 的 `days` 参数完全未生效**：签名/docstring/header 都说 "最近 N 天"，但函数体无任何过滤逻辑。新增 `_parse_news_time` 辅助函数兼容 yfinance 多版本字段（`pubDate` ISO 8601 / `providerPublishTime` Unix 时间戳秒/毫秒），循环内对 `pub_time < cutoff` 的条目跳过；解析失败的条目保留（避免字段变更漏报）。
- **R6-3 `{target_label}` / `{asset_label}` / `{fundamentals_label}` 替换规则与源仓库不符**：README 和 SKILL.md 声称前两者替换为 ticker symbol、第三个替换为字面 `Fundamentals`，但源仓库 `bull_researcher.py:20-25` / `news_analyst.py:17` 实际是 `stock`/`asset`、`company`/`asset`、`Company fundamentals report`/`Asset fundamentals report (may be unavailable for crypto)`。改为与源仓库一致。
- **R6-4 6 个 prompt 的 front-matter "Template variables" 列了 body 中并不存在的变量**：`china_market_analyst.md` / `cn_news_analyst.md` / `market_analyst.md` / `fundamentals_analyst.md` / `news_analyst.md` / `sentiment_analyst.md` 声称的 `{tool_names}` / `{current_date}` / `{instrument_context}` / `{system_message}` 只存在于源仓库外层 `ChatPromptTemplate`，trad-skill 抽取的 body 里没有。front-matter 改为只列出 body 中实际出现的变量；CN 两个 prompt 标 "(none — body is static text)"。

**MEDIUM (8)**

- **R6-5 `compute_indicators` RSI 用 ewm 简化实现**：pandas `ewm(adjust=False)` 递推种子是 `gain[0]`，标准 Wilder RSI 种子是 `mean(gain[1:15])`，偏差约 1pp。文档化为已知限制（注释 + indicators.md 说明），不改代码（完整 Wilder 需 O(n) 循环，风险与收益不匹配）。
- **R6-6 `fetch_cn/hk_stock_data` 中 `start_date.replace` 在 try 块外**：`start_date=None` 时抛 `AttributeError` 穿透函数边界，违反 never raises 契约。两函数顶部加 `if not isinstance(start_date, str) or not isinstance(end_date, str): return "错误: 日期参数无效"` 守卫。
- **R6-7 `compute_indicators` RSI 在持续上涨时返回 NA 而非 100**：`avg_loss.replace(0, pd.NA)` 让 `-0.0` 也变 NA（`-0.0 == 0` 为 True），rs=NA、rsi=NA。改用显式分支：`avg_loss==0` 时 RSI=100，否则标准公式。
- **R6-8 `compute_indicators` Bollinger Bands 用默认 ddof=1**：pandas `rolling().std()` 默认样本标准差（除以 n-1），传统布林带（StockCharts/TradingView）用总体标准差（ddof=0，除以 n），上下轨偏宽约 2.6%。改为 `std(ddof=0)`。
- **R6-9 SKILL.md §4 Stage 6 re-injection 指示绑定 4 份 full report，但 `portfolio_manager.md` body 无对应变量**：`portfolio_manager.md` 只有 `{research_plan}` / `{trader_plan}` / `{history}` / `{lessons_line}`，没有 `{market_research_report}` 等槽位。SKILL.md §4 Stage 6 措辞改为 "append the four full analyst reports as out-of-template context (e.g., prepend as a `## Analyst Reports` section)"。
- **R6-10 SKILL.md §4 CN 市场替换描述把 "2 个" 写成 "3 个"**：Stage 1 总共 4 个分析师，替换 2 个（Market/News）后剩 2 个（Sentiment/Fundamentals），不是 3 个；且 Bull-Bear-Researcher 是 Stage 2 角色不属于 Stage 1。改为 "其余 2 个分析师（Sentiment / Fundamentals）保持不变；Stage 2 及之后的 researcher/manager/risk debator 等角色不受 market 影响"。
- **R6-11 `{instrument_context}` 替换文本与源仓库语义差异**：源仓库 `build_instrument_context` 是含反 hallucination 措辞的段落，trad-skill 改成紧凑单行（token 效率取舍）。README 加 Note 文档化为已知限制，agents 应依赖 SKILL.md §2 ticker 确认作为反幻觉闸门。
- **R6-12 README 声称 "30 unique variables" 但其中 3 个在所有 prompt body 中都不存在**：`{current_date}` / `{tool_names}` / `{system_message}` 只存在于源仓库外层 `ChatPromptTemplate`。README 加 Note 说明这 3 个是 phantom variables（替换是 no-op，但为完整性文档化）。

**LOW (18)**

- **R6-13 `fetch_news.py` 顶部 `from datetime import ...` 死代码**：R6-2 修复后 `_parse_news_time` 用到 `datetime` / `timedelta` / `timezone`，import 不再是死代码。
- **R6-14 `compute_indicators` 的 `_val` 对 inf 返回字符串 "inf"**：`pd.notna(float('inf'))` 为 True，`round(inf, 4)=inf`。VWMA / MFI 分母为 0 时产生 inf。改用 `math.isfinite(float(v))` 同时排除 NaN 与 ±inf，统一返回 "N/A"。
- **R6-15 `fetch_stocktwits` docstring 说 limit 默认 30**：`DEFAULT_SENTIMENT_LIMIT=15`，docstring 与代码不符。改为 "默认 15"。
- **R6-16 `fetch_news` / `fetch_sentiment` 的 `limit` / `days` 参数未钳制负数**：`--limit -1` 时 `news_list[:-1]` / `df.head(-1)` / `messages[:limit]` 切掉最后一条，违背降本设计。各入口函数顶部加 `days = max(1, int(days))` / `limit = max(0, int(limit))`。
- **R6-17 `compute_stats` 注释说 "日对数收益" 实际用 `pct_change`**：注释与代码不一致。改为 "日百分比收益"。
- **R6-18 `compute_indicators` 的 MFI 在所有 tp 持平时返回 NA 而非 50**：同 R6-7 思路处理 0/0 与 X/0 边缘情况：`pos_sum==0 & neg_sum==0` → 50（中性）；`pos_sum>0 & neg_sum==0` → 100（强买）；`pos_sum==0 & neg_sum>0` → 0（强卖）。
- **R6-19 `fetch_reddit_sentiment` / `fetch_stocktwits` 中 symbol 未 URL 编码**：常见股票代码不含 `&` / `=` / `?` 等特殊字符，Reddit/StockTwits API 对 `.` / `-` 容忍。文档化为已知限制（代码注释说明）。
- **R6-20 `fetch_yfinance_news` 中 `content.get` 在 content 非 dict 时抛 AttributeError**：R6-2 修复时已加 `if not isinstance(content, dict): content = {}` 守卫，单条坏数据不再中止整个循环。
- **R6-21 `build_compact_report` 中 `tail=None` 抛 TypeError**：`int(None)` 抛 TypeError，CLI 路径 argparse 不会产生 None，仅直接调用触发。改为 `tail = max(0, int(tail)) if tail is not None else 0`。
- **R6-22 `fetch_cn_fundamentals` / `fetch_cn_stock_data` 的 yfinance 降级未覆盖北交所（8 开头）和 B 股（9 开头）**：会被错误加 `.SZ` 后缀。文档化为已知限制（北交所/B 股流动性低，yfinance 覆盖有限，AKShare 优先路径已覆盖）。
- **R6-23 `market_analyst.md` 的指标列表不含 MFI 但脚本预计算 MFI**：源仓库 `market_analyst.py` 原貌不含 MFI，trad-skill verbatim 继承。indicators.md 加 Note 说明 Market Analyst 应把脚本输出的 MFI 行作为补充指标解读。
- **R6-24 多数非 CN prompt 在 `{get_language_instruction()}` 前多了一个空行**：源仓库 `+ get_language_instruction()` 无换行，trad-skill 加了空行（whitespace 美化）。文档化为已知限制（对 LLM 几乎无影响）。
- **R6-25 `trader.md` 的 2-block 结构未在 SKILL.md §5 Stage 4 说明**：trader.md 有 `## System Message` 和 `## User Message` 两个独立代码块。SKILL.md §5 Stage 4 加 Note 说明需用两个 role 构造 LLM 调用。
- **R6-26 `{get_language_instruction()}` 在 English 场景下的替换值与源仓库不一致**：源仓库 English 时返回空字符串，trad-skill README 声称总是替换为 `Respond in <language> per output_language.`。README 改为：English → 空字符串；非 English → ` Write your entire response in <lang>.`（贴近源行为）。
- **R6-27 SKILL.md §6 表格未列出 `--indicators` / `--no-indicators` / `--no-stats` 三个 argparse flag**：fetch_stock_data.py 行的描述补一句 "Flags: `--indicators`/`--no-indicators` (default on), `--stats`/`--no-stats` (default off), `--raw` (legacy full CSV), `--tail N` (default 30)"。
- **R6-28 `install.mjs` `destDir` 仍用 `path.join`**：round-5 修了 `scriptsDir` 用 `path.resolve`，但漏了 `destDir`。`--dir ./foo` 时显示相对路径。改为 `path.resolve(parentDir, SKILL_NAME)`。
- **R6-29 `README.md` 配置表 `market` 默认值 `...` 应为 `—`**：与 SKILL.md / README_CN.md 一致（em dash 表示 "无默认值，自动检测"）。
- **R6-30 `README_CN.md` 港股 "5位数字" 与同文件 L29 4位数字示例矛盾**：L29 用 `0700.HK, 9988.HK`，L136 说 "5位数字"。改为 "4-5 位数字 + `.HK` 后缀（如 0700.HK 或 00700.HK；脚本 `zfill(5)` 两种都接受）"。

### Changed

- `package.json` `version`: `1.3.5` → `1.3.6`。
- 新增 `verify_round6.py`：32 项检查（30 bug + AST 语法 + 副本一致性），全部通过。

## [1.3.5] - 2026-07-26

### Fixed (round 5)

- **9 个非 CN prompt 缺 `{get_language_instruction()}`**：源仓库 TradingAgents 在 12 个非 CN prompt 末尾都有 `+ get_language_instruction()`，本 skill 早期只把 3 个（sentiment_analyst / trader / portfolio_manager）转成模板变量，其余 9 个被剥离。这同时是 verbatim 违反（AGENTS.md "Do NOT paraphrase"）和功能性 bug：`output_language` 配置只对 3/12 非 CN 子代理生效，其余 9 个永远收不到 "Respond in <language>" 指令。在 market_analyst / news_analyst / fundamentals_analyst / bull_researcher / bear_researcher / research_manager / aggressive_risk / conservative_risk / neutral_risk 的 prompt body 末尾（closing ``` 前）补 `{get_language_instruction()}`，front-matter "Template variables" 行同步更新。CN prompts 正确地不含此变量（源 CN 仓库没有）。
- **`fetch_news(None)` / `fetch_stock_data(None,...)` / `fetch_sentiment(None)` 抛 AttributeError**：round-4 已硬化 `fetch_fundamentals`，但另外 3 个统一入口和内部 helper `fetch_stock_df` 仍直接调 `symbol.strip()`/`symbol.upper()`，主代理跳过某 ticker 传 None 时抛 AttributeError，违反 "never raises, returns a string" 契约。镜像 fetch_fundamentals 模式：入口加 `isinstance(symbol, str)` + `strip()` + 空串守卫返回错误字符串；`fetch_stock_df` 同守卫返回空 DataFrame（其文档化失败模式）。
- **`npm pack` 把 `__pycache__/*.pyc` 打进 tarball**：`package.json` `files` allowlist 包含 `tradingagents-analysis/` 和 `skills/` 目录，npm 11 实测在 `files` 存在时根目录 `.npmignore` 不会从 allowlist 中再减项（与 agentsync #120 报告一致）。在 `files` 内追加 `!**/__pycache__/**` / `!**/*.pyc` / `!**/*.pyo` 否定项；同时加 `prepublishOnly` 脚本，发布前清理两份 `scripts/__pycache__`（防御性，确保 `npm publish` 时即使有残留 .pyc 也不会进入 tarball）。
- **`install.mjs` `--dir` 相对路径打印相对路径**：L130 `path.join(scriptsDir, s)` 在 `--dir ./foo` 时输出 `./foo/tradingagents-analysis/scripts/fetch_stock_data.py`，子代理 CWD 不在仓库根会找不到。改为 `path.resolve(scriptsDir, s)`，始终打印绝对路径，并用 `/` 分隔便于跨平台复制粘贴。
- **`install.mjs` `mkdirSync` 在 try/catch 之外**：L104 `fs.mkdirSync(parentDir, { recursive: true })` 在权限不足 / 路径非法时抛裸 Node 堆栈。移入 L107 既有 try/catch，走 `fail()` 友好提示。
- **`install.mjs` `--dir` + `--agent` 互斥未检查**：同时指定时静默走 `--agent` 分支，`--dir` 被忽略，用户期望装到 `--dir` 却装到 `~/.config/opencode/skills`。加 `if (args.dir && args.agent) fail('不能同时指定 --dir 和 --agent')`。
- **`README_CN.md` A股说明 `.SS`/`.SZ` 残留**：L129 仍写 "`.SS` 后缀为上海，`.SZ` 后缀为深圳"，与 round-2 已统一的"6 位纯数字 → A股"规则矛盾（用户不需要也不应该手动加后缀）。改为 "脚本内部根据 6 位代码前缀自动判断交易所（6 开头 → 上海 .SS；0/3 开头 → 深圳 .SZ），用户只需提供 6 位纯数字"。
- **`README.md` / `README_CN.md` 相对链接 404**：两份 README 的数据源章节用 `[references/data-sources.md](references/data-sources.md)` 相对链接，但 README 在仓库根，目标实际在 `tradingagents-analysis/references/data-sources.md`，GitHub 渲染 404。改为 `tradingagents-analysis/references/data-sources.md`。
- **`prompts/README.md` `{investment_plan}` 流向错误**：L104 写 "Research Manager output → Trader input + Risk Debate input"，但 Risk Debate 接收的是 `{trader_decision}`/`{trader_plan}`（L105 已正确），不接收 `{investment_plan}`。改为 "Research Manager output → Trader input. (Risk Debate does NOT receive `{investment_plan}` — it receives `{trader_decision}`/`{trader_plan}` per the row below.)"
- **`prompts/README.md` Tool-Name Override 章节未标注 CN prompt 不引用 ghost tools**：L126 / L132 标题写 `market_analyst.md / china_market_analyst.md`、`news_analyst.md / cn_news_analyst.md`，让读者以为 CN 文件也引用 ghost tools。实际 CN prompts（grep 验证）不引用任何 `get_*` 工具，数据源描述是 inline 的（akshare 行情 / 新闻）。两处标题下各加一行 Note 说明 override 仅适用于 EN prompt。
- **`fetch_fundamentals.py` `_fmt_num` 对 `pd.NA` 返回 `'<NA>'` 而非 `'N/A'`**：旧守卫 `isinstance(v, float) and pd.isna(v)` 只捕获 float NaN，`pd.NA`（类型 `pandas._libs.missing.NAType`，不是 float）落到 `round(float(v), 2)` 抛 TypeError，被 except 捕获后返回 `str(v) = '<NA>'`，导致表格里 'N/A' 与 '<NA>' 两种缺失标记混排。改为 `v is None or pd.isna(v)`，覆盖 None / np.nan / pd.NA / NaT 四种缺失形态统一返回 'N/A'。
- **`CHANGELOG.md` "9 个 ghost tools" 计数错误**：1.3.4 章节列了 9 个 ghost tools 但实际是 11 个（Market 3 + News 4 + Fundamentals 4）。改为 11。
- **`.npmignore` 缺 `node_modules/` 和 `CLAUDE.md`**：补两行，作为 `files` allowlist 被移除时的 fallback。同时加备注说明：当 `files` 存在时根目录 `.npmignore` 不会被 npm 11 用于从 allowlist 减项，主防御在 `package.json` 的 `!` 否定项 + `prepublishOnly` 脚本。

### Changed

- `package.json` `version`: `1.3.4` → `1.3.5`。

## [1.3.4] - 2026-07-25

### Fixed (round 4)

- **`fetch_fundamentals("")` / `fetch_fundamentals(None)` 抛异常**：`fetch_us_fundamentals` 的 `yf.Ticker(symbol)` 在 try/except 之外，空串触发 `ValueError: Empty ticker name`；`fetch_fundamentals` 的 `symbol.isdigit()` 对 None 触发 `AttributeError`。违反 AGENTS.md "never raises" 契约。入口加 `isinstance(symbol, str)` + `strip()` + 空串守卫返回错误字符串；`yf.Ticker(symbol)` 移入既有 try/except（与 `fetch_us_stock_data` 一致）。
- **`compute_stats` 输出 "年化波动率: nan%"**：单行 DataFrame（或 `pct_change().dropna()` 不足 2 行）时 `vol` 为 NaN，`round(float(vol), 2)` 输出 `"nan%"`，对 LLM 有误导（看似数值）。引入 `_num()` helper，NaN/非数值 → "N/A"，应用于 `ret`/`vol`/`hi`/`lo`（与 `avg_vol` 处理一致）。
- **`README.md` / `README_CN.md` 示例日期与 SKILL.md §6 矛盾**：Round 3 BUG 7 把 SKILL.md §6 示例扩到 1 年（`2023-07-01` 至 `2024-06-30`），但漏改两份 README，仍残留 `2024-01-01 --end 2024-06-30`（6 个月）和 `2024-01-01 --end 2024-01-31`（1 个月）。用户复制 README 示例运行会得到 SMA200=N/A。两份 README 的指标示例同步为 `2023-07-01 --end 2024-06-30`；raw 示例扩到 6 个月。
- **`sentiment_analyst.md` 的 `{news_block}` 无数据源**：verbatim 提示词期望 `{news_block}` / `{stocktwits_block}` / `{reddit_block}` 三块预取数据，但 SKILL.md spawn 只让跑 `fetch_sentiment.py`（无新闻），`{news_block}` 孤立。在 `prompts/README.md` 文档化映射：`{stocktwits_block}`/`{reddit_block}` → 脚本输出对应段落；`{news_block}` → 脚本不提供，用 web-search fallback 或留空（News Analyst 单独覆盖新闻）。
- **verbatim 提示词 30 个模板变量未文档化替换规则**：`{ticker}`、`{current_date}`、`{instrument_context}`、`{get_language_instruction()}`、`{NO_EXTERNAL_TOOLS}`、`{tool_names}`、`{system_message}`、`{target_label}`、`{company_name}`、`{asset_label}` 等 30 个 LangChain 风格变量，SKILL.md 从未指导主代理替换，子代理收到字面量 `{...}`。新增 `prompts/README.md` "Template Variable Substitution" 章节，5 个子表（Identity / Dates / Context-language-tooling / Data reports / Pre-fetched blocks）覆盖全部 30 个变量；SKILL.md §4 加指针。
- **`--no-stats` 参数缺失**：`--indicators` 配对了 `--no-indicators`，但 `--stats` 只有 store_true 无配对 `--no-stats`。用户尝试 `--no-stats` 会触发 argparse `unrecognized arguments`。补 `--no-stats`（dest="stats", action="store_false"）。
- **`prompts/README.md` Tool-Name Override 只覆盖 `market_analyst.md`**：Round 3 BUG 9 只列 `get_stock_data`/`get_indicators`/`get_verified_market_snapshot`，漏掉 `news_analyst.md` 的 `get_news`/`get_global_news`/`get_macro_indicators`/`get_prediction_markets` 和 `fundamentals_analyst.md` 的 `get_fundamentals`/`get_balance_sheet`/`get_cashflow`/`get_income_statement`。扩展章节覆盖全部 11 个 ghost tools，标注哪些有脚本映射、哪些需 web-search fallback。

### Changed

- `package.json` `version`: `1.3.2` → `1.3.4`（跳过 1.3.3 避免与 round-3 PR 冲突）。

## [1.3.2] - 2026-07-25

### Fixed (round 3)

- **`fetch_stock_data.py` `--start`/`--end` 与文档默认值矛盾**：argparse 设为 `required=True`，但 SKILL.md §6 L242 文档说有默认值（"默认 `--start` 取 trade date 前 1 年"）。代理按文档省略参数时 argparse 直接拒绝运行。改为可选：`--end` 默认今天、`--start` 默认今天往前 365 天（覆盖 SMA200 所需 ~200 个交易日）。SKILL.md §6 L242 同步改写为"若未传则脚本默认取今天往前 1 年到今天；如需分析历史交易日，请显式传"。
- **`fetch_cn_sentiment` 全部数据源失败时返回裸 `<unavailable>`**：Round 1 修复了 US 分支的裸 `<unavailable>`（用 `> 数据源不可用` 友好块包裹），但 CN 分支被遗漏 —— `fetch_cn_sentiment` 在 `has_data == False` 时 `return "<unavailable>"`，丢弃已构建的结构化错误块。改为返回累积的 `sections`（含 `> akshare 未安装，跳过` / `> 获取失败` 等）+ `> A 股情绪数据源全部不可用` 汇总。
- **`data-sources.md` CN 新闻降级链顺序反了**：文档写 `Google News (Chinese) → AKShare news`，但 `fetch_cn_news` 实际是 AKShare 优先（`stock_news_em` / 东方财富）、Google News 兜底。改为 `AKShare news (stock_news_em / 东方财富) → Google News (Chinese)`。
- **SKILL.md §3 News Analyst 过度声称**：表格 "Key inputs" 列声称 `fetch_news.py` 提供 "global macro, FRED indicators, prediction markets"，但脚本只抓公司新闻。Round 2 修了 §6 但漏了 §3。改为 "Company news via `fetch_news.py` (FRED / Polymarket / macro: web-search fallback only — not in script)"。
- **`install.mjs` L97 过时注释**：注释说 "若不存在则回退到 ~/.agents/skills"，但代码无任何回退逻辑。删除误导性半句，保留 `// 默认 Claude Code`。
- **SKILL.md §3 Sentiment Analyst 过度声称 "news headlines"**：`fetch_sentiment.py` 只做 StockTwits+Reddit（US）和 akshare 个股评论+机构参与度（CN），不抓 news headlines。Focus 列改为 "Social sentiment → composite score"，Key inputs 列改为 "StockTwits, Reddit (US) / akshare (CN) via `fetch_sentiment.py`"。
- **SKILL.md §6 示例时间窗口短于 SMA200 所需**：示例 `--start 2024-01-01 --end 2024-06-30`（6 个月）与 L242 "至少需 200 个交易日才能算 SMA200"（≈10 个月）矛盾。扩到 1 年：`--start 2023-07-01 --end 2024-06-30`。
- **`fetch_stock_data.py` 负数 `--tail` 触发未捕获 ValueError**：`build_compact_report` 中 `df.tail(tail)` 在 tail 为负时抛 ValueError，向上传播到 CLI 导致脚本崩溃，违反 AGENTS.md "never raises" 契约。入口处加 `tail = max(0, int(tail))` 钳制。
- **`prompts/README.md` 未文档化工具名 override**：`market_analyst.md` 等 verbatim 提示词引用 `get_stock_data` / `get_indicators` / `get_verified_market_snapshot`（不存在），SKILL.md §4 与 `indicators.md` 已有 override 但 `prompts/README.md` 未说明。新增 "Tool-Name Override" 章节解释映射关系，不改 verbatim prompt 本身。

### Changed

- `package.json` `version`: `1.3.1` → `1.3.2`。

## [1.3.1] - 2026-07-25

### Fixed (round 1, commit 7c958ec — previously undocumented)

- **`install.mjs` OpenCode 安装路径不匹配**：`--agent opencode` 实际安装到 `~/.config/opencode/skills`，但旧代码装到 `~/.opencode/skills`。已对齐 SKILL.md 文档。
- **`fetch_sentiment.py` 美股情绪报告裸露 `<unavailable>` 占位符**：当 StockTwits/Reddit 不可用时直接拼接裸字符串。已改为类似 `fetch_news.py` 的友好错误提示块（`> 数据源不可用`）。
- **`fetch_fundamentals.py` `_fmt_num()` 返回类型不一致**：类型注解为 `str`，但对数值返回 `float`。已统一返回 `str`。
- **`fetch_stock_data.py` docstring 拼写错误**：`_normalize_ohlcv` 中 "olumnolume" → "Volume"。
- **A股函数缺少显式 `ak is not None` 检查**：`fetch_cn_fundamentals` 和 `fetch_cn_sentiment` 在调用 akshare API 前未显式检查，与其他脚本模式不一致。已补齐。

### Fixed (round 2, this release)

- **`fetch_fundamentals.py` `_yoy()` 计算方向反了**：yfinance `financials` 列为降序（最近年在前），但代码用 `iloc[-2]`/`iloc[-1]` 取最旧两年。改为 `iloc[0]`/`iloc[1]` 取最近一年同比。同步修 docstring。
- **A 股基本面/情绪章节静默失败**：`df.to_markdown()` 惰性依赖未声明的 `tabulate` 包，未安装时 A 股整套表格输出变为 `> 获取失败: Import tabulate failed.`。`fetch_fundamentals.py` 和 `fetch_sentiment.py` 中所有 `to_markdown(index=False)` 改为 `to_string(index=False)`（纯 pandas，无新依赖）。
- **`fetch_stock_data.py` `build_compact_report` 失败路径重复网络请求**：失败时为拿错误字符串再次调用 `fetch_stock_data`。重构为单次调用 + 本地 CSV 解析。
- **4 个脚本冗余 `import sys`**：全程未引用，已删除。
- **`install.mjs` 把 `__pycache__/*.pyc` 复制到用户目录**：`fs.cpSync` 加 `filter` 排除 `__pycache__`。
- **`install.mjs` `--dir`/`--agent` 缺值时静默走默认路径**：缺值或值以 `--` 开头时 `fail()` 并打印友好错误。支持 `--dir=PATH`/`--agent=NAME` 等号语法。未知参数 `fail()`。
- **`install.mjs` `--dir ~/foo` 不展开 `~`**：在 PowerShell/cmd 中 `~` 不会自动展开，会在当前目录创建名为 `~` 的垃圾目录。已用 `os.homedir()` 展开 `~` 和 `~/`。
- **`install.mjs` `cpSync`/`rmSync` 无错误处理**：失败时抛原始 Node 堆栈。已包 `try/catch` 走 `fail()`。
- **SKILL.md spawn 模板硬编码脚本名和 `--start/--end`**：导致 News/Fundamentals/Sentiment 分析师脚本报 `unrecognized arguments`。模板改为 `{script_name}`/`{script_args}` 占位符，注释明确按 §6 替换脚本名**和**参数。
- **SKILL.md 未定义 `--start`/`--end` 默认窗口**：代理不知道取多长历史窗口。§6 加指引：默认 trade date 前 1 年到当天（至少 200 个交易日才能算 SMA200）。
- **CN 专用 prompt 文件存在但 SKILL.md 完全没引用**：`china_market_analyst.md` 和 `cn_news_analyst.md` 是 A股/港股 专用 prompt，但 SKILL.md 没说何时切换。§4 加 CN market prompt swap 说明。
- **A股自动检测规则文档与脚本不符**：README/SKILL.md 写 `.SS`/`.SZ` 后缀 → A股，但脚本只识别 6 位纯数字。文档统一改为「6 位纯数字（如 600519、000858）→ A股」。
- **SKILL.md §3 语法错误**：`Stages 1 uses parallel sub-agents` → `Stage 1 uses parallel sub-agents`。
- **SKILL.md §6 表格过度声称**：`fetch_news.py` 描述去掉 "macro"，`fetch_sentiment.py` 描述去掉 "headline analysis"（脚本不做这些）。
- **README OpenCode 安装路径错误**：`--agent opencode` 注释从 `~/.opencode/skills` 改为 `~/.config/opencode/skills`，与 install.mjs 一致。
- **README_CN A股数据源优先级错误**：`Tushare → AKShare → Baostock` → `AKShare → yfinance`（脚本实际行为）。
- **README Project Structure tree 缺 `skills/` 目录且有 `└──` 重复**：补 `skills/` 目录行，修掉连续 `└──`。
- **`references/data-sources.md` 降级链与脚本不符**：A股 `MongoDB cache → Tushare → AKShare → Baostock → TDX` → `AKShare → yfinance`；美股 `yfinance → Alpha Vantage` → `yfinance`；美股新闻 `Yahoo Finance News → Alpha Vantage News → Google News` → `yfinance + Google News`。Configuration 章节注明属原始框架，本 skill 不读取。未实现的源（Alpha Vantage/FRED/Polymarket/Tushare/Baostock/TDX）标注 "not wired in scripts"。
- **`references/prompts/README.md` 含开发机绝对路径**：`D:\niaod\RustroverProjects\trad\...` 改为 GitHub 仓库链接。
- **`references/prompts/README.md` 漏列 MFI 指标**：Market Analyst 指标列表补 MFI。
- **`references/prompts/README.md` "with Tushare data" 误导**：改为 `with akshare data (Tushare referenced in prompt but not wired in scripts)`。
- **`references/prompts/README.md` 5 阶段 vs 6 阶段不一致**：加注 `Decision = Research Manager + Trader; see SKILL.md §3 for the full 6-stage flow`。
- **第一轮修复未入 CHANGELOG，version 未 bump**：本次 1.3.1 补齐第一轮和第二轮所有 Fixed 条目。

### Added

- `.npmignore`：排除 `__pycache__/`、`*.pyc`、`*.pyo`、`.omo/`、`.codegraph/`、`.trae/`、`*.log`、`.vscode/`，作为 `package.json` `files` allowlist 的兜底。
- `.gitattributes`：`* text=auto eol=lf`，统一行尾符，防止两份 `tradingagents-analysis/` 拷贝之间 CRLF/LF 漂移。
- `AGENTS.md` Gotchas：补充 `.trae/specs/` 是 spec 工作流状态（已跟踪，与 `.omo/` 不同）的说明。
- `.gitignore`：补 `.codegraph/`（防御性，不依赖嵌套 .gitignore）和 `node_modules/`（future-proof）。
- SKILL.md §6：`fetch_stock_data.py` 行后加 `--start`/`--end` 默认窗口指引。
- SKILL.md §4：CN market prompt swap 说明（A股/港股时用 `china_market_analyst.md` / `cn_news_analyst.md`）。

### Changed

- `package.json` `version`: `1.3.0` → `1.3.1`。

## [1.3.0] - 2026-07-24

### Added
- Standard installation via [vercel-labs/skills](https://github.com/vercel-labs/skills) CLI: `npx skills add halfoffive/trad-skill` now works across 70+ coding agents (Claude Code, Cursor, Windsurf, Trae, OpenCode, Codex, etc.).
- `skills/tradingagents-analysis/` directory: the standard location for skills CLI discovery. Contains a full copy of the skill (identical to the root copy).
- "For AI Agents: Installation Guide" section at the top of SKILL.md with explicit install commands, path locations, and dependency notes.
- "For AI Agents" subsection in both READMEs with agent-specific install instructions.
- Manual installation instructions using raw.githubusercontent.com URLs (curl-based copy-paste commands).

### Changed
- READMEs (English + Chinese): Installation section restructured — `npx skills add` is now the recommended method, custom `npx halfoffive/trad-skill` installer retained as fallback.
- `package.json`: `files` array now includes `skills/` directory for proper npm/npx packing.
- AGENTS.md: updated structure diagram and install command docs to reflect dual-location layout.

### Notes
- Backward compatible: the existing `npx halfoffive/trad-skill` custom installer continues to work unchanged.
- Root `tradingagents-analysis/` is preserved as an identical copy for the custom installer; both locations are maintained.
- Python scripts, prompts, and core pipeline logic are unchanged.

## [1.2.0] - 2026-07-23

### Changed — Deep token-cost reduction (let the LLM use python scripts)

- **Indicator computation moved into the script.** `fetch_stock_data.py` now pre-computes SMA(50/200), EMA(10), MACD/signal/hist, RSI(14), Bollinger(20,2), ATR(14), VWMA(20), MFI(14) with pure pandas (no new deps) and prints a compact indicator snapshot (latest values + trend signals: golden/death cross, overbought/oversold, band position). The Market Analyst now **interprets** pre-computed values instead of doing arithmetic over a 250-row CSV. Resolves the SKILL.md §6 "Market Analyst computes indicators" cost center.
- **Data output compacted across all scripts** to shrink the payloads that enter analyst reports and then get re-injected downstream:
  - `fetch_stock_data.py`: default output is OHLCV **tail** (default `--tail 30`, was full range) + indicators + optional `--stats`; `--raw` preserves the legacy full-range CSV.
  - `fetch_fundamentals.py`: curated **compact key-metrics table** (revenue, net income, EPS, FCF, debt, equity, OCF, margins + YoY) replaces `to_markdown()` dumps of all 4-year × 3-statement line items.
  - `fetch_news.py`: default `--limit 8` per source (was 20×2=40); all summaries truncated to 200 chars (was US-only); per-item format slimmed to title + source + one-line summary.
  - `fetch_sentiment.py`: default `--limit 15` (was 30); displayed messages 15→8; Reddit posts 20→8.
- **SKILL.md re-injection discipline (biggest lever).** Analyst reports must be concise (≤~400 words) and lead with a `## Key Signals` digest (5–8 bullets). The Stage 2 & 5 debate prompts now bind the four analyst reports to their **Key Signals digests** instead of full bodies (the verbatim prompt bodies are unchanged — only what is bound to the `{*_report}` variables changes). The Stage 6 Portfolio Manager still receives full reports + transcript once.
- `SKILL.md §4` spawn template now tells sub-agents the python script IS the data source / "verified snapshot" and not to attempt nonexistent tool names (`get_stock_data` / `get_indicators` / `get_verified_market_snapshot`), cutting wasted tool-call round-trips.
- `SKILL.md §7` final reasoning capped to 3–4 concise paragraphs (cite, don't re-narrate).
- `references/indicators.md`: notes that indicators are pre-computed by the script; the "Verified Market Snapshot" section now points at the script output instead of the nonexistent `get_verified_market_snapshot` tool.

### Added
- `fetch_stock_data.py`: `--tail`, `--indicators` / `--no-indicators`, `--stats`, `--raw` flags; `compute_indicators()` and `compute_stats()` helpers; `build_compact_report()` default entry; `_normalize_ohlcv()` shared normalizer.
- `fetch_fundamentals.py`: `_build_us_metric_table()` curated key-metrics extractor with YoY.
- `fetch_news.py`: `_truncate()` helper; `--limit` flag (default 8); slim `_format_news_item()`.

## [1.1.0] - 2026-07-23

### Added
- `package.json` + `install.mjs`: a self-contained, zero-dependency npx installer. `npx halfoffive/trad-skill` copies the skill into the target agent's skills directory (default `~/.claude/skills/tradingagents-analysis`), with `--dir` and `--agent claude|agents|opencode` overrides. Idempotent.
- SKILL.md §2 "Before You Start — Confirm the Target": the agent now **asks the user which ticker to analyze** (and optionally date / debate rounds) before spawning any sub-agent, instead of assuming a ticker was given.
- SKILL.md §4: the main agent now resolves the absolute path to the skill's `scripts/` directory and embeds it in each sub-agent spawn, with a spawn template that requires running the script before writing the report.

### Fixed
- **Sub-agents never ran the bundled scripts** — SKILL.md told them to run `scripts/{script}.py` (a relative path), but a sub-agent's working directory is the user's project, not the skill folder, so the scripts never resolved. Now invoked by absolute path.
- Stale risk-debate prompt filenames in SKILL.md: `aggressive_analyst.md` / `conservative_analyst.md` / `neutral_analyst.md` → `aggressive_risk.md` / `conservative_risk.md` / `neutral_risk.md` (matching the actual files).
- `fetch_sentiment.py` ignored its `--limit` flag (parsed but never passed through). Now wired into `fetch_stocktwits`.
- `fetch_fundamentals.py` and `fetch_sentiment.py` imported `akshare` unconditionally, crashing on machines without it. Now wrapped in `try/except ImportError → ak = None` with graceful degradation, matching the other scripts.
- SKILL.md / README overclaims corrected: `fetch_stock_data.py` returns raw OHLCV (the Market Analyst computes indicators per `references/indicators.md`); `fetch_news.py` covers company + macro news (FRED / prediction-market claims removed since unimplemented).
- **`npx skill add …` never worked** — the third-party `skill` CLI (vercel-labs/codebuddy) has no `add` subcommand and no `owner/repo/subdir` support. Replaced by the custom `npx halfoffive/trad-skill` installer.

### Changed
- Scripts are now the **primary** data source; web search / browser tools are a fallback only for parts a script could not provide — no longer an easy excuse to skip the scripts.
- Default install target is `~/.claude/skills` (Claude Code); manual-install examples updated accordingly.
- SKILL.md sections renumbered (added §2, shifted the rest) with cross-references updated.

## [1.0.0] - 2026-07-22

### Added
- SKILL.md: Multi-agent trading analysis pipeline with sub-agent orchestration
- 14 verbatim agent role prompts in `references/prompts/` (from TradingAgents + TradingAgents-CN)
- Data source catalog (`references/data-sources.md`): 12 sources covering US, A-share, and HK markets
- Technical indicator reference (`references/indicators.md`): 13 indicators across 5 categories
- Python data-fetching scripts (functional style, Chinese comments):
  - `fetch_stock_data.py` — OHLCV via yfinance + akshare
  - `fetch_news.py` — news via yfinance + Google News RSS + akshare
  - `fetch_fundamentals.py` — financials via yfinance + akshare
  - `fetch_sentiment.py` — sentiment via StockTwits + Reddit + akshare
- Bilingual documentation: README.md (English) + README_CN.md (中文)
- AGENTS.md for AI agent onboarding
- Language switch buttons in both READMEs
- `npx skill add` one-click install support

### Design Decisions
- Skill files live in `tradingagents-analysis/` subfolder (clean repo root for meta files)
- Prompts are verbatim extracts — never paraphrased from memory
- Scripts return formatted strings (for LLM prompt injection), never raise exceptions
- No class-based Python; functional style throughout
