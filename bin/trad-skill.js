#!/usr/bin/env node
// trad-skill unified launcher: 极薄的 Node 分发器。
// 唯一职责：解析当前平台的预编译二进制（@trad-skill/<platform> optionalDependency），
// 然后 exec 它。所有逻辑（安装器 + 数据工具）均由 Rust 二进制实现
// （crates/trad-data/src/，产出的二进制名为 trad-skill）。
//
// 无参数 / 非数据子命令 → 默认 install 模式，注入 --skills-dir。
// 数据子命令 (stock/news/fundamentals/sentiment/fund) → 原样透传，不注入 --skills-dir
// （按首个位置参数判断，允许 `--source yahoo stock` 这种 flag 在子命令之前的形态）。
// -V/--version/-h/--help → 原样透传，不注入 install（显示顶层 CLI，而非 install 子命令）。
// 显式 `install` 同样注入 --skills-dir：Rust 二进制回落的 CARGO_MANIFEST_DIR
// 路径是构建机路径，在用户机器上不存在，必须显式给出包内技能目录。
//
// bunx trad-skill@latest                         → 安装到 ~/.agents/skills（默认）
// bunx trad-skill@latest --agent claude
// bunx trad-skill@latest install                 → 显式安装（同样注入 --skills-dir）
// bunx trad-skill@latest stock --symbol AAPL     → 直接取数
// npx trad-skill@latest                          → 同上（fallback）
const { execFileSync } = require('node:child_process');
const path = require('node:path');

const platform = process.platform; // win32, darwin, linux
const arch = process.arch; // x64, arm64
const platformDir = `${platform}-${arch}`;
const ext = platform === 'win32' ? '.exe' : '';
const binName = `trad-skill${ext}`;

// npm optionalDependency（bunx / npx / node_modules 上下文）
let npmBinary = null;
try {
  npmBinary = require.resolve(`@trad-skill/${platformDir}/${binName}`);
} catch {
  // 非 npm 上下文，或平台包未安装
}

if (!npmBinary) {
  console.error(`trad-skill: 未找到 ${platformDir} 平台的二进制文件。`);
  console.error('  重新安装: bunx trad-skill@latest  (或 npx trad-skill@latest)');
  console.error(`  或手动安装平台包: npm install @trad-skill/${platformDir}`);
  process.exit(1);
}

// 数据子命令：这些子命令下，launcher 不注入 --skills-dir（透传用户参数）。
const DATA_SUBCOMMANDS = new Set(['stock', 'news', 'fundamentals', 'sentiment', 'fund']);
// 顶层 CLI flag（-V/--version/-h/--help）：原样透传，不注入 install，否则
// `--version` 会变成 trad-skill-install --version、`--help` 只显示 install 子命令。
const TOP_LEVEL_FLAGS = new Set(['-V', '--version', '-h', '--help']);
// 携带值的选项：其后的 token 是选项的值、不是位置参数（M15 扫描与 M1 剥离都要跳过）。
const VALUE_FLAGS = new Set([
  '--dir', '--agent', '--skills-dir', '--bin-path', // install 模式
  '--symbol', '--source', '--start', '--end', '--tail', '--limit', '--days', // data 模式
]);
const userArgs = process.argv.slice(2);

// M15: 跳过前导选项（含选项值），取首个位置参数判断是否为数据子命令。
// 这样 `--source yahoo stock` 也能识别为数据调用并原样透传。注：Rust CLI 本身
// 仍不接受 flag 在子命令之前（二进制层已文档化的限制），launcher 只是不误路由进 install。
function firstPositional(args) {
  let prevWasValueFlag = false;
  for (const a of args) {
    if (prevWasValueFlag) {
      prevWasValueFlag = false;
      continue;
    }
    if (VALUE_FLAGS.has(a)) {
      prevWasValueFlag = true;
      continue;
    }
    if (!a.startsWith('--')) return a;
  }
  return null;
}

// M1: 剥离任意位置的 `install` token，除非它是某选项的值（如 `--dir install`，
// 此时 install 是目录名、必须保留）。
function stripInstallTokens(args) {
  const out = [];
  let prevWasValueFlag = false;
  for (const a of args) {
    if (prevWasValueFlag) {
      out.push(a);
      prevWasValueFlag = false;
      continue;
    }
    if (VALUE_FLAGS.has(a)) {
      prevWasValueFlag = true;
      out.push(a);
      continue;
    }
    if (a === 'install') continue;
    out.push(a);
  }
  return out;
}

// 包根目录（tarball 布局：<pkg>/{bin/, skills/, ...}）→ 注入源技能目录。
// 显式 `install` 同样注入：Rust 侧 clap 对重复 --skills-dir 是后者胜，
// 用户自带的 --skills-dir（在注入参数之后）会被保留。
const pkgRoot = path.resolve(__dirname, '..');
const skillsDir = path.join(pkgRoot, 'skills', 'tradingagents-analysis');

const firstPos = firstPositional(userArgs);
const forwardedArgs =
  TOP_LEVEL_FLAGS.has(userArgs[0]) || (firstPos && DATA_SUBCOMMANDS.has(firstPos))
    ? userArgs
    : ['install', '--skills-dir', skillsDir, ...stripInstallTokens(userArgs)];

try {
  execFileSync(npmBinary, forwardedArgs, { stdio: 'inherit' });
} catch (e) {
  if (e.status !== undefined && e.status !== null) {
    process.exit(e.status);
  }
  if (e.signal) {
    const sig = require('os').constants.signals[e.signal];
    process.exit(sig ? 128 + sig : 1);
  }
  if (e.code === 'ENOENT') {
    console.error(`trad-skill binary not found at ${npmBinary}. Please reinstall: bunx trad-skill@latest`);
    process.exit(1);
  }
  if (e.code === 'EACCES') {
    console.error(`trad-skill binary at ${npmBinary} is not executable.`);
    console.error(`  Try: chmod +x "${npmBinary}"`);
    process.exit(1);
  }
  throw e;
}
