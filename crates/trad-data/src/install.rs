//! 技能安装器：把 tradingagents-analysis/ 技能复制到目标 AI agent 的 skills 目录。
//!
//! 安装逻辑全部用 Rust 实现，npm 包 `trad-skill` 仅保留一个极薄的 JS launcher
//! 负责解析平台二进制并 exec 本程序。
//!
//! 默认目标：`~/.agents/skills`（通用 agent 目录）。可用 `--agent claude|opencode` 或 `--dir` 覆盖。
//! 无子命令时默认进入安装流程；`stock` / `news` / `fundamentals` / `sentiment` 是数据子命令。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;

/// 技能名（同时也是目标目录下的子目录名）
pub const SKILL_NAME: &str = "tradingagents-analysis";

/// 支持的预置 AI agent 目标
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum AgentTarget {
    /// 安装到 ~/.claude/skills（Claude Code）
    Claude,
    /// 安装到 ~/.agents/skills（通用 agent 目录，默认）
    Agents,
    /// 安装到 ~/.config/opencode/skills（OpenCode）
    Opencode,
}

/// `trad-skill install` 子命令参数（与顶层平铺参数保持一致）
#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    /// 目标 agent：claude | agents | opencode（默认 agents）
    #[arg(long, value_enum, conflicts_with = "dir")]
    pub agent: Option<AgentTarget>,

    /// 自定义目标 skills 父目录（与 --agent 互斥）
    #[arg(long, conflicts_with = "agent")]
    pub dir: Option<String>,

    /// 源技能目录（含 SKILL.md 的 tradingagents-analysis 目录）。
    /// 生产环境由 JS launcher 注入；dev 下默认相对 CARGO_MANIFEST_DIR。
    /// `overrides_with` 自引用：launcher 注入一个默认值后，用户自带的
    /// --skills-dir 是后者胜，而不是报"参数重复"。
    #[arg(long = "skills-dir", overrides_with = "skills_dir")]
    pub skills_dir: Option<String>,

    /// 要复制的平台二进制路径。默认取当前运行的可执行文件（self-copy）。
    #[arg(long = "bin-path")]
    pub bin_path: Option<String>,

    /// 跳过复制平台二进制
    #[arg(long = "no-bin")]
    pub no_bin: bool,

    /// 只打印安装计划，不写入任何文件
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// 兼容历史：要复制的 wrapper（已忽略，仅保留以避免旧 launcher 报错）。
    #[arg(long = "wrapper", hide = true)]
    pub wrapper: Option<String>,
}

/// 读取 home 目录：Unix 优先 HOME；Windows 优先 USERPROFILE。
///
/// Git Bash / MSYS2 / CI shell 在 Windows 上常把 HOME 设为 POSIX 风格路径
/// （如 `/c/Users/foo`），Rust 会把前导 `/` 解析为当前盘符根目录，导致技能被
/// 静默装到 `C:\c\Users\foo\.agents\skills` 这类错误位置。因此 Windows 下仅当
/// HOME 是盘符绝对路径（`C:\...`）时才采用，否则回退 USERPROFILE。
/// 不引入 `dirs` crate——windows/unix 路径分支覆盖全部 7 个交叉编译目标。
fn home_dir() -> Option<PathBuf> {
    if std::env::consts::OS == "windows" {
        std::env::var_os("USERPROFILE")
            .filter(|v| !v.is_empty())
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|h| !h.is_empty() && is_drive_letter_abs(h.as_os_str()))
            })
            .map(PathBuf::from)
    } else {
        std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
            .map(PathBuf::from)
    }
}

/// HOME 是否为盘符绝对路径（`C:\...` / `C:/...`）
fn is_drive_letter_abs(p: &std::ffi::OsStr) -> bool {
    let s = p.to_string_lossy();
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/')
}

/// 预置 agent → 默认 skills 父目录
fn agent_dir(target: &AgentTarget) -> Result<PathBuf> {
    let home =
        home_dir().ok_or_else(|| anyhow!("无法确定 home 目录（HOME/USERPROFILE 未设置）"))?;
    Ok(match target {
        AgentTarget::Claude => home.join(".claude").join("skills"),
        AgentTarget::Agents => home.join(".agents").join("skills"),
        AgentTarget::Opencode => home.join(".config").join("opencode").join("skills"),
    })
}

/// 把相对路径解析为相对 cwd 的绝对路径（不要求路径已存在）
fn absolutize(p: &str) -> Result<PathBuf> {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

/// 展开 `~` / `~/` / `~\` 为 home 目录前缀
fn expand_tilde(p: &str) -> Result<PathBuf> {
    let home =
        || home_dir().ok_or_else(|| anyhow!("无法确定 home 目录（HOME/USERPROFILE 未设置）"));
    if p == "~" {
        return home();
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return Ok(home()?.join(rest));
    }
    if let Some(rest) = p.strip_prefix("~\\") {
        return Ok(home()?.join(rest));
    }
    absolutize(p)
}

/// 解析目标 skills 父目录。--dir 与 --agent 互斥；二者皆空时默认 agents。
fn resolve_parent_dir(dir: &Option<String>, agent: &Option<AgentTarget>) -> Result<PathBuf> {
    match (dir, agent) {
        (Some(_), Some(_)) => {
            bail!("不能同时指定 --dir 和 --agent（--dir 为自定义路径，--agent 为预置目标）")
        }
        (Some(d), None) => expand_tilde(d),
        (None, Some(a)) => agent_dir(a),
        (None, None) => agent_dir(&AgentTarget::Agents),
    }
}

/// 把 rustc 的 target 元素映射为 Node 的 process.platform/process.arch 命名。
/// `std::env::consts` 在编译期按 target 固化，因此交叉编译产物在目标机上运行时返回正确值。
/// 该键必须与 npm optionalDependency 包名 `@trad-skill/<key>` 一致
/// （launcher `bin/trad-skill.js` 通过 `require.resolve('@trad-skill/<key>/...')`
/// 解析平台二进制），否则将静默找不到二进制——务必保持同步并配测试。
fn node_platform_key() -> String {
    let plat = match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        "linux" => "linux",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{plat}-{arch}")
}

/// 平台二进制文件扩展名
fn bin_ext() -> &'static str {
    if std::env::consts::OS == "windows" {
        ".exe"
    } else {
        ""
    }
}

/// 两个路径是否指向同一目录（两者都存在时用 canonicalize 消除符号链接差异）
fn same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// 纯 std 递归目录复制（不引入额外运行时依赖；技能目录全是文本文件，无符号链接）
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("创建目录失败：{}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("读取目录失败：{}", src.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else if ft.is_file() {
            fs::copy(&path, &dest).with_context(|| {
                format!("复制文件失败：{} → {}", path.display(), dest.display())
            })?;
        }
        // 符号链接：技能目录不包含，跳过
    }
    Ok(())
}

/// 测试注入：记录一个 tmp_dir 路径，`attempt_swap` 遇到相同路径时伪造"目标已存在"
/// 的 AlreadyExists，供 `test_install_atomic_rollback` 确定性触发回滚路径。
/// 按路径匹配而非全局开关，避免与并行运行的其他 install 测试互相干扰。
/// 仅测试构建存在；生产构建中该标志与相关分支均不编译。
#[cfg(test)]
static FORCE_SWAP_FAIL: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// swap 阶段执行 rename(tmp_dir → dest_dir)。
///
/// 真实文件系统上无法确定性构造"两个 rename 之间目标被非空目录占用"的状态
/// （第二步执行前 dest 已被第一步移走；Windows 上目标已存在非空目录即失败），
/// 因此回滚路径由测试注入触发：`FORCE_SWAP_FAIL` 记录本测试的 tmp_dir 路径后，
/// 本函数伪造 `ErrorKind::AlreadyExists`，与 Windows 上"rename 覆盖非空目录"
/// 的真实错误同型。
fn attempt_swap(tmp_dir: &Path, dest_dir: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    {
        let mut guard = FORCE_SWAP_FAIL.lock().unwrap();
        if guard.as_ref().is_some_and(|p| p == tmp_dir) {
            *guard = None;
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("注入失败：目标目录已存在 {}", dest_dir.display()),
            ));
        }
    }
    fs::rename(tmp_dir, dest_dir)
}

/// 交换安装目录：③ 旧目录改名备份 → ④ 新目录换名就位 → ⑤ 全部成功后才清理备份。
///
/// 若 ④ 失败（磁盘满、权限、杀软锁、测试注入），回滚把备份恢复为 dest_dir，
/// 旧安装保持完好；回滚自身失败只打印警告，不掩盖原始错误。
fn swap_dirs(tmp_dir: &Path, dest_dir: &Path, backup_dir: &Path) -> Result<()> {
    // ③ 旧目录改名备份
    if dest_dir.exists() {
        fs::rename(dest_dir, backup_dir)
            .with_context(|| format!("备份旧安装失败：{}", dest_dir.display()))?;
    }
    // ④ 新目录换名就位
    if let Err(e) = attempt_swap(tmp_dir, dest_dir) {
        // 回滚：恢复旧安装，避免留下空目标目录
        if backup_dir.exists() {
            if let Err(rollback_err) = fs::rename(backup_dir, dest_dir) {
                eprintln!(
                    "⚠ 回滚失败：旧安装仍在 {}（{}",
                    backup_dir.display(),
                    rollback_err
                );
            }
        }
        // 清理临时目录，避免残留
        let _ = fs::remove_dir_all(tmp_dir);
        return Err(e).context(format!("技能文件就位失败：{}", dest_dir.display()));
    }
    // ⑤ 全部成功后才删备份（二进制已随 tmp_dir 一并就位）
    if backup_dir.exists() {
        let _ = fs::remove_dir_all(backup_dir); // 备份清理失败仅留残留，不阻断安装
    }
    Ok(())
}

/// 安装入口
pub fn run(args: InstallArgs) -> Result<()> {
    // 1. 解析源技能目录
    let skills_src = match &args.skills_dir {
        Some(s) => expand_tilde(s)?,
        None => {
            // dev 默认：相对 crate manifest 指向仓库 skills/
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../skills")
                .join(SKILL_NAME)
        }
    };
    let skill_md = skills_src.join("SKILL.md");
    if !skill_md.is_file() {
        bail!(
            "找不到技能源目录：{}（缺少 SKILL.md，安装包可能不完整）",
            skills_src.display()
        );
    }

    // 2. 解析目标父目录
    let parent_dir = resolve_parent_dir(&args.dir, &args.agent)?;
    let dest_dir = parent_dir.join(SKILL_NAME);

    // 防御：--skills-dir 指向已安装副本时 src 与 dest 相同，下面的删除会自毁 → 直接拒绝
    if same_path(&skills_src, &dest_dir) {
        bail!(
            "源技能目录与目标目录相同（{}）：请改用 --dir 指定其它安装目标",
            dest_dir.display()
        );
    }

    let node_key = node_platform_key();
    let ext = bin_ext();
    let bin_name = format!("trad-skill{ext}");

    // 3. 解析二进制源（self-copy：当前可执行文件即平台二进制）
    let bin_src = if args.no_bin {
        None
    } else {
        match &args.bin_path {
            Some(b) => Some(PathBuf::from(b)),
            None => Some(std::env::current_exe()?),
        }
    };

    // 4. dry-run：只打印计划
    if args.dry_run {
        println!("【试运行】将安装 {SKILL_NAME} → {}", dest_dir.display());
        println!("  源技能目录：{}", skills_src.display());
        match &bin_src {
            Some(b) => println!(
                "  二进制：{} → {}/{bin_name}",
                b.display(),
                dest_dir.join("bin").join(&node_key).display()
            ),
            None => println!("  二进制：跳过（--no-bin）"),
        }
        return Ok(());
    }

    // 5. 真正安装：幂等 + 数据安全。
    //    - 目标已存在但缺 SKILL.md → 不是本技能安装，拒绝删除（防误删用户目录）
    //    - 流程：① 技能文件与平台二进制都复制到同目录临时目录；② 旧目录改名备份；
    //      ③ 新目录换名就位；④ 全部成功后才删备份。任一步失败（磁盘满、权限、
    //      杀软锁、--bin-path 无效）时 dest_dir 都不动，上一次的可用安装完整保留。
    fs::create_dir_all(&parent_dir)
        .with_context(|| format!("创建父目录失败：{}", parent_dir.display()))?;
    if dest_dir.exists() && !dest_dir.join("SKILL.md").is_file() {
        bail!(
            "目标目录已存在但不是本技能安装（缺少 SKILL.md）：{}。\n\
             请确认后手动删除该目录，或改用其它 --dir / --agent 目标。",
            dest_dir.display()
        );
    }
    let tmp_dir = parent_dir.join(format!(".{SKILL_NAME}.tmp-{}", std::process::id()));
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)
            .with_context(|| format!("清理残留临时目录失败：{}", tmp_dir.display()))?;
    }

    // ① 技能文件 → tmp_dir
    copy_dir_recursive(&skills_src, &tmp_dir).context("复制技能文件失败")?;

    // ② 平台二进制 → tmp_dir/bin/<node_key>/trad-skill[.exe]
    //    必须在 swap 之前完成：复制失败（权限、杀软锁、--bin-path 无效）时直接中止，
    //    dest_dir 未被触碰，旧安装完整保留。
    if !args.no_bin {
        let b = bin_src
            .as_ref()
            .ok_or_else(|| anyhow!("无法确定平台二进制路径"))?;
        if !b.exists() {
            let _ = fs::remove_dir_all(&tmp_dir);
            bail!(
                "找不到平台二进制：{}。安装已中止，目标目录未改动，原有安装保持不变。",
                b.display()
            );
        }
        let tmp_platform_dir = tmp_dir.join("bin").join(&node_key);
        if let Err(e) = fs::create_dir_all(&tmp_platform_dir)
            .and_then(|_| fs::copy(b, tmp_platform_dir.join(&bin_name)))
        {
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(e).context(format!("复制平台二进制失败：{}", b.display()));
        }
    }

    // ③ 旧目录改名备份 → ④ 新目录换名就位 → ⑤ 成功后清理备份（swap_dirs 内含回滚）
    let backup_dir = parent_dir.join(format!(".{SKILL_NAME}.old-{}", std::process::id()));
    swap_dirs(&tmp_dir, &dest_dir, &backup_dir)?;

    // 6. 打印结果。二进制已随 tmp_dir 一并就位，仅 --no-bin 时未装
    println!("✓ 已安装 {SKILL_NAME} → {}", dest_dir.display());
    println!();
    println!("下一步：");
    if args.no_bin {
        println!("  ⚠ 数据二进制未安装（--no-bin），可改用 `bunx trad-skill@latest <子命令>` 调用数据工具。");
    } else {
        println!("  ✓ trad-skill 二进制已安装 ({node_key})");
    }
    println!("  1. 重启你的 AI agent / 开一个新会话，让它加载该技能。");
    println!("  2. 触发分析，例如：\"分析 AAPL\" 或 \"Analyze 600519\" 。");
    println!();
    println!("数据工具（Rust 二进制）:");
    println!("  bunx trad-skill@latest stock --symbol AAPL");
    println!("  bunx trad-skill@latest fundamentals --symbol AAPL");
    println!("  bunx trad-skill@latest news --symbol AAPL");
    println!("  bunx trad-skill@latest sentiment --symbol AAPL");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// 这些测试会写进程级 HOME/USERPROFILE 环境变量；cargo 默认并行跑测试，
    /// 用此锁串行化所有改环境变量的测试，避免互相覆盖导致偶发失败。
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// 跨平台设置 home（HOME + USERPROFILE），返回临时 home 路径
    fn set_home(tmp: &Path) {
        std::env::set_var("HOME", tmp);
        std::env::set_var("USERPROFILE", tmp);
    }

    fn default_install_args() -> InstallArgs {
        InstallArgs {
            agent: None,
            dir: None,
            skills_dir: None,
            bin_path: None,
            no_bin: true,
            dry_run: false,
            wrapper: None,
        }
    }

    /// 构造显式 dir + skills_dir 的安装参数（no_bin，避免测试触碰真实二进制）
    fn install_args(dir: &str, skills_dir: &str) -> InstallArgs {
        InstallArgs {
            dir: Some(dir.to_string()),
            skills_dir: Some(skills_dir.to_string()),
            ..default_install_args()
        }
    }

    /// 建一个含 SKILL.md 的最小技能源目录
    fn make_skill_src(root: &Path) -> std::path::PathBuf {
        let src = root.join("src");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("SKILL.md"), "# test skill").unwrap();
        std::fs::write(src.join("nested/file.txt"), "a").unwrap();
        src
    }

    #[test]
    fn rejects_src_equals_dest() {
        let root = std::env::temp_dir().join("trad-skill-install-srcdest");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        // 技能源直接位于目标同名目录（模拟 --skills-dir 指向已安装副本）
        let src = root.join(SKILL_NAME);
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("SKILL.md"), "# test skill").unwrap();
        // --dir <root> → dest == <root>/tradingagents-analysis == src
        let args = install_args(root.to_str().unwrap(), src.to_str().unwrap());
        let err = run(args).unwrap_err();
        assert!(err.to_string().contains("相同"), "应拒绝 src==dest: {err}");
        // 源目录未被破坏
        assert!(src.join("SKILL.md").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_existing_dir_without_skill_marker() {
        let root = std::env::temp_dir().join("trad-skill-install-noskill");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let src = make_skill_src(&root);
        // 目标已存在同名目录但不含 SKILL.md（用户数据）
        let dest = root.join(SKILL_NAME);
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("user-data.txt"), "keep me").unwrap();
        let args = install_args(root.to_str().unwrap(), src.to_str().unwrap());
        let err = run(args).unwrap_err();
        assert!(
            err.to_string().contains("SKILL.md"),
            "应提示缺 SKILL.md: {err}"
        );
        // 用户目录未被删除
        assert!(dest.join("user-data.txt").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_is_idempotent() {
        let root = std::env::temp_dir().join("trad-skill-install-idem");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let src = make_skill_src(&root);
        let target = root.join("target");
        // 首次安装
        run(install_args(
            target.to_str().unwrap(),
            src.to_str().unwrap(),
        ))
        .unwrap();
        let installed = target.join(SKILL_NAME);
        assert!(installed.join("SKILL.md").exists());
        // 重复安装（幂等）也成功，且内容被替换
        std::fs::write(src.join("SKILL.md"), "# test skill v2").unwrap();
        run(install_args(
            target.to_str().unwrap(),
            src.to_str().unwrap(),
        ))
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(installed.join("SKILL.md")).unwrap(),
            "# test skill v2"
        );
        // 无残留临时/备份目录
        for entry in std::fs::read_dir(&target).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(
                name == SKILL_NAME,
                "target 下不应有残留目录: {}",
                name.to_string_lossy()
            );
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_bin_path_fails_install() {
        let root = std::env::temp_dir().join("trad-skill-install-nobin");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let src = make_skill_src(&root);
        let target = root.join("target");
        let args = InstallArgs {
            dir: Some(target.to_str().unwrap().to_string()),
            skills_dir: Some(src.to_str().unwrap().to_string()),
            bin_path: Some(root.join("nonexistent-bin").to_str().unwrap().to_string()),
            no_bin: false,
            ..default_install_args()
        };
        let err = run(args).unwrap_err();
        assert!(
            err.to_string().contains("二进制"),
            "应报告二进制未安装: {err}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_install_bin_copy_failure_preserves_old() {
        // TDD 用例(H3):bin 复制失败时旧安装必须原样保留。
        // 旧实现先 swap 再复制二进制:bin_path 无效时,旧安装已被新技能文件
        // 替换且备份被删除 → 本测试应失败(旧 SKILL.md / 旧 bin 丢失)。
        let root = std::env::temp_dir().join("trad-skill-install-bin-copy-fail");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let src = make_skill_src(&root);
        let target = root.join("target");
        let dest = target.join(SKILL_NAME);
        // 预置旧安装:SKILL.md + 旧平台二进制
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("SKILL.md"), "# OLD INSTALL").unwrap();
        let old_bin = dest
            .join("bin")
            .join(node_platform_key())
            .join(format!("trad-skill{}", bin_ext()));
        std::fs::create_dir_all(old_bin.parent().unwrap()).unwrap();
        std::fs::write(&old_bin, "old binary bytes").unwrap();
        // bin_path 指向不存在的文件 → 二进制复制必然失败
        let args = InstallArgs {
            dir: Some(target.to_str().unwrap().to_string()),
            skills_dir: Some(src.to_str().unwrap().to_string()),
            bin_path: Some(root.join("nonexistent-bin").to_str().unwrap().to_string()),
            no_bin: false,
            ..default_install_args()
        };
        let err = run(args).unwrap_err();
        assert!(
            err.to_string().contains("二进制"),
            "应报告二进制问题: {err}"
        );
        // 旧安装原样保留:SKILL.md 与旧 bin 都可读
        assert_eq!(
            std::fs::read_to_string(dest.join("SKILL.md")).unwrap(),
            "# OLD INSTALL"
        );
        assert_eq!(std::fs::read(&old_bin).unwrap(), b"old binary bytes");
        // 无残留 tmp 目录
        let tmp_dir = target.join(format!(".{SKILL_NAME}.tmp-{}", std::process::id()));
        assert!(!tmp_dir.exists(), "失败后不应残留 tmp 目录");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_install_atomic_rollback() {
        // swap(tmp→dest) 失败时必须回滚：旧安装恢复为 dest_dir。
        // 注入方式：置位 FORCE_SWAP_FAIL（test-only 静态标志），attempt_swap 会伪造
        // ErrorKind::AlreadyExists —— 与 Windows 上"rename 覆盖已存在非空目录"的真实
        // 失败同型（见 attempt_swap 注释：真实文件系统无法确定性构造两步 rename 之间
        // 目标被占用的状态）。若删掉 swap_dirs 里的回滚分支，本测试立即失败。
        let root = std::env::temp_dir().join("trad-skill-install-rollback");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let src = make_skill_src(&root);
        let target = root.join("target");
        let dest = target.join(SKILL_NAME);
        // 预置旧安装
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("SKILL.md"), "# OLD INSTALL").unwrap();
        std::fs::write(dest.join("old-file.txt"), "old data").unwrap();

        let tmp_dir = target.join(format!(".{SKILL_NAME}.tmp-{}", std::process::id()));
        // 注入：只有本次安装的 tmp_dir 会命中，并行跑的其他 install 测试不受影响
        *FORCE_SWAP_FAIL.lock().unwrap() = Some(tmp_dir.clone());
        let err = run(install_args(
            target.to_str().unwrap(),
            src.to_str().unwrap(),
        ))
        .unwrap_err();
        *FORCE_SWAP_FAIL.lock().unwrap() = None; // 兜底复位（正常路径在 attempt_swap 内已消费）
        assert!(
            err.to_string().contains("就位失败"),
            "应报告 swap 失败: {err}"
        );
        // 回滚生效：旧安装恢复为 dest_dir，backup 无残留
        assert_eq!(
            std::fs::read_to_string(dest.join("SKILL.md")).unwrap(),
            "# OLD INSTALL"
        );
        assert!(dest.join("old-file.txt").exists());
        let backup_dir = target.join(format!(".{SKILL_NAME}.old-{}", std::process::id()));
        assert!(!backup_dir.exists(), "回滚后 backup 应已恢复回 dest");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn agent_dir_mapping() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = std::env::temp_dir().join("trad-skill-install-test-agentdir");
        std::fs::remove_dir_all(&tmp).ok();
        std::fs::create_dir_all(&tmp).unwrap();
        set_home(&tmp);
        assert_eq!(
            agent_dir(&AgentTarget::Claude).unwrap(),
            tmp.join(".claude/skills")
        );
        assert_eq!(
            agent_dir(&AgentTarget::Agents).unwrap(),
            tmp.join(".agents/skills")
        );
        assert_eq!(
            agent_dir(&AgentTarget::Opencode).unwrap(),
            tmp.join(".config/opencode/skills")
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn default_target_is_agents() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = std::env::temp_dir().join("trad-skill-install-test-default");
        std::fs::remove_dir_all(&tmp).ok();
        std::fs::create_dir_all(&tmp).unwrap();
        set_home(&tmp);
        let resolved = resolve_parent_dir(&None, &None).unwrap();
        assert_eq!(resolved, tmp.join(".agents/skills"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn dir_and_agent_mutually_exclusive() {
        use crate::Cli;
        use clap::Parser;
        let r = Cli::try_parse_from(["trad-skill", "install", "--dir", "x", "--agent", "claude"]);
        assert!(r.is_err(), "--dir 与 --agent 应互斥");
    }

    #[test]
    fn expand_tilde_basic() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = std::env::temp_dir().join("trad-skill-install-test-tilde");
        std::fs::remove_dir_all(&tmp).ok();
        std::fs::create_dir_all(&tmp).unwrap();
        set_home(&tmp);
        assert_eq!(expand_tilde("~").unwrap(), tmp);
        assert_eq!(expand_tilde("~/foo").unwrap(), tmp.join("foo"));
        assert_eq!(expand_tilde("~\\bar").unwrap(), tmp.join("bar"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn copy_dir_recursive_roundtrip() {
        let root = std::env::temp_dir().join("trad-skill-copytest");
        std::fs::remove_dir_all(&root).ok();
        let src = root.join("src");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("a.txt"), "hello").unwrap();
        std::fs::write(src.join("nested/b.txt"), "world").unwrap();
        let dst = root.join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "hello");
        assert_eq!(
            std::fs::read_to_string(dst.join("nested/b.txt")).unwrap(),
            "world"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn node_platform_key_shape() {
        let key = node_platform_key();
        let parts: Vec<&str> = key.split('-').collect();
        assert_eq!(parts.len(), 2, "键应为 <os>-<arch> 形式");
        assert!(
            matches!(parts[0], "win32" | "darwin" | "linux"),
            "platform 部分 {0} 应为 win32/darwin/linux",
            parts[0]
        );
        assert!(
            matches!(parts[1], "x64" | "arm64"),
            "arch 部分 {0} 应为 x64/arm64",
            parts[1]
        );
    }

    #[test]
    fn top_level_install_flags_survive_explicit_subcommand() {
        use crate::Cli;
        use clap::Parser;
        // 顶层 --agent 在显式 install 子命令下不应丢失（合并逻辑）
        let cli = Cli::try_parse_from(["trad-skill", "--agent", "claude", "install"]).unwrap();
        let args = cli.into_install_args();
        assert!(matches!(args.agent, Some(AgentTarget::Claude)));
        // 子命令级显式参数优先
        let cli = Cli::try_parse_from(["trad-skill", "install", "--agent", "opencode"]).unwrap();
        let args = cli.into_install_args();
        assert!(matches!(args.agent, Some(AgentTarget::Opencode)));
        // 无子命令：顶层 flag 生效
        let cli = Cli::try_parse_from(["trad-skill", "--dry-run"]).unwrap();
        let args = cli.into_install_args();
        assert!(args.dry_run);
    }

    #[test]
    fn no_subcommand_defaults_to_install() {
        use crate::Cli;
        use clap::Parser;
        let cli = Cli::try_parse_from(["trad-skill"]).expect("无子命令应解析成功");
        assert!(cli.command.is_none());
        // into_install_args 应能从顶层 flags 构造
        let args = default_install_args();
        assert!(args.agent.is_none());
    }

    #[test]
    fn data_subcommand_passes_through() {
        use crate::Cli;
        use clap::Parser;
        let cli = Cli::try_parse_from(["trad-skill", "stock", "--symbol", "AAPL"])
            .expect("stock 子命令应解析成功");
        assert!(matches!(cli.command, Some(crate::Commands::Stock { .. })));
    }
}
