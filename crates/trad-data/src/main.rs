mod format;
mod fund;
mod fundamentals;
mod http;
mod indicators;
mod install;
mod market;
mod news;
mod sentiment;
mod yahoo;

use anyhow::anyhow;
use chrono::{Duration, Local, NaiveDate};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "trad-skill",
    about = "TradingAgents skill: installer + data fetcher",
    version,
    propagate_version = true
)]
struct Cli {
    /// 目标 agent：claude | agents | opencode（默认 agents）。仅 install 模式生效。
    #[arg(long, value_enum, global = true, conflicts_with = "dir")]
    agent: Option<install::AgentTarget>,

    /// 自定义目标 skills 父目录（与 --agent 互斥）。仅 install 模式生效。
    #[arg(long, global = true, conflicts_with = "agent")]
    dir: Option<String>,

    /// 源技能目录。仅 install 模式生效。
    /// `overrides_with` 自引用：launcher 注入默认值后用户自带的 --skills-dir 后者胜。
    #[arg(long = "skills-dir", global = true, overrides_with = "skills_dir")]
    skills_dir: Option<String>,

    /// 要复制的平台二进制路径。默认 self-copy。仅 install 模式生效。
    #[arg(long = "bin-path", global = true)]
    bin_path: Option<String>,

    /// 跳过复制平台二进制。仅 install 模式生效。
    #[arg(long = "no-bin", global = true)]
    no_bin: bool,

    /// 只打印安装计划，不写入。仅 install 模式生效。
    #[arg(long = "dry-run", global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 获取行情数据 (兼容 fetch_stock_data.py)
    Stock {
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
        #[arg(long, default_value_t = 30)]
        tail: u32,
        #[arg(long, default_value_t = true)]
        indicators: bool,
        #[arg(long = "no-indicators")]
        no_indicators: bool,
        #[arg(long, default_value_t = false)]
        stats: bool,
        #[arg(long = "no-stats")]
        no_stats: bool,
        #[arg(long, default_value_t = false)]
        raw: bool,
        /// 数据渠道：yahoo | eastmoney（默认按市场自动选择）
        #[arg(long, value_enum)]
        source: Option<market::Source>,
    },
    /// 获取基本面数据 (兼容 fetch_fundamentals.py)
    Fundamentals {
        #[arg(long)]
        symbol: String,
    },
    /// 获取新闻数据 (兼容 fetch_news.py)
    News {
        #[arg(long)]
        symbol: String,
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long, default_value_t = 8)]
        limit: u32,
    },
    /// 获取情绪数据 (兼容 fetch_sentiment.py)
    Sentiment {
        #[arg(long)]
        symbol: String,
        #[arg(long, default_value_t = 15)]
        limit: u32,
        /// Reddit 帖子时间窗（天，默认 7）。仅美股/加密的 Reddit 源生效；
        /// ≤7 天按 week、>7 天按 month 过滤。
        #[arg(long, default_value_t = 7)]
        days: u32,
    },
    /// 获取公募基金数据 (A股基金：净值/资料/重仓股/业绩)
    Fund {
        #[arg(long)]
        symbol: String,
        #[arg(long, default_value_t = 30)]
        tail: u32,
        #[arg(long, default_value_t = 365)]
        days: u32,
    },
    /// 安装 tradingagents-analysis 技能（无子命令时的默认行为）
    Install {
        #[command(flatten)]
        args: install::InstallArgs,
    },
}

impl Cli {
    /// 把顶层平铺的 install flags 收集为 InstallArgs。
    /// 显式 `install` 子命令优先，但子命令未显式给定的字段回落到顶层 flag，
    /// 避免 `trad-skill --agent claude install` 把顶层 --agent 静默丢掉。
    fn into_install_args(self) -> install::InstallArgs {
        match self.command {
            Some(Commands::Install { args }) => install::InstallArgs {
                agent: args.agent.or(self.agent),
                dir: args.dir.or(self.dir),
                skills_dir: args.skills_dir.or(self.skills_dir),
                wrapper: args.wrapper,
                bin_path: args.bin_path.or(self.bin_path),
                no_bin: args.no_bin || self.no_bin,
                dry_run: args.dry_run || self.dry_run,
            },
            _ => install::InstallArgs {
                agent: self.agent,
                dir: self.dir,
                skills_dir: self.skills_dir,
                wrapper: None,
                bin_path: self.bin_path,
                no_bin: self.no_bin,
                dry_run: self.dry_run,
            },
        }
    }
}

/// 校验并规范化日期范围（YYYY-MM-DD）。
/// end < start、start 晚于今天、格式非法 → Err（CLI 参数错误，退出码 2）。
fn validate_date_range(start: &str, end: &str) -> Result<(String, String), String> {
    let parse = |s: &str| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| format!("日期格式无效（应为 YYYY-MM-DD）：{}", s))
    };
    let s = parse(start)?;
    let e = parse(end)?;
    if e < s {
        return Err(format!("--end（{}）早于 --start（{}）", end, start));
    }
    if s > Local::now().date_naive() {
        return Err(format!("--start（{}）晚于今天", start));
    }
    Ok((start.to_string(), end.to_string()))
}

/// 以非零码退出：1 = 取数/网络失败，2 = 参数错误
fn exit_with(code: i32, msg: &str) -> ! {
    eprintln!("{}", msg);
    std::process::exit(code);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 无子命令，或显式 `install` → 走安装器（无需 HTTP 客户端）
    let is_install = matches!(cli.command, None | Some(Commands::Install { .. }));
    if is_install {
        let args = cli.into_install_args();
        install::run(args)?;
        return Ok(());
    }

    let client = http::build_client()?;

    match cli.command {
        Some(Commands::Stock {
            symbol,
            start,
            end,
            tail,
            indicators,
            no_indicators,
            stats,
            no_stats,
            raw,
            source,
        }) => {
            let use_indicators = indicators && !no_indicators;
            let use_stats = stats && !no_stats;

            // 默认日期：end=今天（本地时区，避免 UTC 在凌晨把"今天"算成昨天），
            // start=今天往前 365 天；再统一校验（格式 / end>=start / 未来日期）。
            let today = Local::now().format("%Y-%m-%d").to_string();
            let end_date = end.unwrap_or_else(|| today.clone());
            let start_date = start.unwrap_or_else(|| {
                (Local::now() - Duration::days(365))
                    .format("%Y-%m-%d")
                    .to_string()
            });
            let (start_date, end_date) = validate_date_range(&start_date, &end_date)
                .unwrap_or_else(|e| exit_with(2, &format!("错误: {}", e)));

            // --tail 限制在 [1, 500]，避免 --tail 0 输出空块或巨量输出
            let tail = tail.clamp(1, 500);

            if raw {
                // --raw 模式：纯 CSV 输出
                match market::fetch_ohlcv(&client, &symbol, &start_date, &end_date, source).await {
                    Ok(data) => {
                        if data.is_empty() {
                            exit_with(1, &format!("错误: 未获取到 {} 的数据", symbol));
                        } else {
                            print!("{}", format::ohlcv_to_csv(&data));
                        }
                    }
                    Err(e) => exit_with(1, &e),
                }
            } else {
                // 默认模式：精简报告（指标 + 尾部 OHLCV）
                match market::fetch_ohlcv(&client, &symbol, &start_date, &end_date, source).await {
                    Ok(data) => {
                        if data.is_empty() {
                            // 空数据按取数失败处理（exit 1），而不是打印"错误"报告后 exit 0
                            exit_with(1, &format!("错误: 未获取到 {} 的数据", symbol));
                        }
                        let opts = format::ReportOptions {
                            tail,
                            indicators: use_indicators,
                            stats: use_stats,
                        };
                        let report = format::build_compact_report(
                            &symbol,
                            &start_date,
                            &end_date,
                            &data,
                            &opts,
                        );
                        print!("{}", report);
                    }
                    Err(e) => exit_with(1, &e),
                }
            }
        }
        Some(Commands::Fundamentals { symbol }) => {
            match fundamentals::fetch_fundamentals(&client, &symbol).await {
                Ok(out) => println!("{}", out),
                Err(e) => exit_with(1, &e),
            }
        }
        Some(Commands::News {
            symbol,
            days,
            limit,
        }) => {
            // --limit 限制在 [1, 1000]，避免单源单次抓取输出巨量新闻
            if limit == 0 || limit > 1000 {
                exit_with(2, "错误: --limit 必须在 1-1000 之间");
            }
            match news::fetch_news(&client, &symbol, days, limit).await {
                Ok(out) => println!("{}", out),
                Err(e) => exit_with(1, &e),
            }
        }
        Some(Commands::Sentiment {
            symbol,
            limit,
            days,
        }) => {
            // --limit 限制在 [1, 1000]，避免单源单次抓取输出巨量帖子
            if limit == 0 || limit > 1000 {
                exit_with(2, "错误: --limit 必须在 1-1000 之间");
            }
            match sentiment::fetch_sentiment(&client, &symbol, limit, days).await {
                Ok(out) => println!("{}", out),
                Err(e) => exit_with(1, &e),
            }
        }
        Some(Commands::Fund { symbol, tail, days }) => {
            // 基金代码必须为 6 位纯数字（基金代码与股票代码冲突，子命令名携带类型）
            let s = symbol.trim();
            if s.len() != 6 || !s.chars().all(|c| c.is_ascii_digit()) {
                exit_with(2, "错误: 基金代码必须为 6 位纯数字");
            }
            let tail = tail.clamp(1, 365);
            // --days 限制在 [1, 3650]（10 年上限），避免超大值产生远古起始日期
            let days = days.clamp(1, 3650);
            match fund::fetch_fund(&client, s, tail, days).await {
                Ok(out) => println!("{}", out),
                Err(e) => exit_with(1, &e),
            }
        }
        Some(Commands::Install { .. }) | None => {
            return Err(anyhow!("内部错误：安装流程不应进入数据分发路径"));
        }
    }

    Ok(())
}
