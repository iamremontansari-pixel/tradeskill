# AGENTS.md

## What this repo is

An AI agent **skill** (not an application). It teaches an AI agent to run the TradingAgents multi-agent stock analysis pipeline. Installable via `bunx trad-skill` (default target: `~/.agents/skills`); `npx trad-skill` works as a fallback.

The core deliverable is `skills/tradingagents-analysis/` (SKILL.md + references/ in the repo; the installer adds bin/<platform>/ at install time). Rust source lives in `crates/trad-data/` — the compiled binary is named `trad-skill` and serves as both the **data tool** and the **installer** (default action when invoked with no subcommand) — with CI (cargo fmt/clippy/test + 7-platform cross-build) in `.github/workflows/ci.yml`.

## Structure

```
trad-skill/                        # repo root (meta files + installer)
├── package.json                  # npm entry point (name: trad-skill)
├── bin/trad-skill.js             # thin JS launcher -> Rust binary (install + data)
├── README.md / README_CN.md       # bilingual docs with language switch
├── CHANGELOG.md                   # version history
├── AGENTS.md                      # this file
├── CONTRIBUTING.md                # branch/commit/release conventions
├── CLAUDE.md                      # pointer to AGENTS.md (do not edit)
├── LICENSE                        # Apache 2.0
├── .github/workflows/ci.yml      # CI: fmt + clippy + test + 7-platform build
├── .github/workflows/release.yml  # tag-triggered release: 7-platform build + npm publish
├── crates/trad-data/              # Rust source (binary name: trad-skill)
├── npm/                           # 5 platform package dirs (@trad-skill/*)
└── skills/
    └── tradingagents-analysis/    # the installable skill
        ├── SKILL.md               # skill entry point (YAML frontmatter)
        ├── references/prompts/    # role prompts for each analyst
        ├── references/data-sources.md
        └── references/indicators.md
```

## Install commands

Recommended (Rust installer via bunx; default target `~/.agents/skills`):
```bash
bunx trad-skill@latest              # install to ~/.agents/skills (default)
bunx trad-skill@latest --agent claude   # install to ~/.claude/skills
bunx trad-skill@latest --dry-run        # print plan, write nothing
```

Fallback (no `bun`): `npx trad-skill@latest` behaves identically.

Data tool without installing: `bunx trad-skill@latest stock --symbol AAPL` (also `news` / `fundamentals` / `sentiment` / `fund`). The same `trad-skill` binary handles both install and data subcommands.

Deprecated: the third-party `npx skills add halfoffive/trad-skill ...` flow (vercel-labs/skills) still works but is no longer recommended.

## Installer (`package.json` + `bin/trad-skill.js` + Rust binary)

The installer logic is **Rust**, in `crates/trad-data/src/install.rs`. The compiled binary is named `trad-skill`; invoking it with no subcommand defaults to the install flow, and an explicit `install` subcommand is also accepted. The npm `trad-skill` bin entry is a thin CJS launcher (`bin/trad-skill.js`) that resolves the platform binary and execs it. Data subcommands (`stock`/`news`/`fundamentals`/`sentiment`/`fund`) are passed through verbatim; for install mode (implicit or explicit `install`), the launcher injects `--skills-dir <pkgRoot>/skills/tradingagents-analysis` — the Rust binary's built-in default is a build-machine path and must never be used on user machines. A user-supplied `--skills-dir` wins over the injected one (clap self-override).

- Copies `skills/tradingagents-analysis/` into the target agent's skills dir. **Default target: `~/.agents/skills`** (generic agent directory). Flags: `--dir <path>`, `--agent claude|agents|opencode` (mutually exclusive).
- Copies the platform binary into `bin/<node-platform-key>/` via **self-copy** (`std::env::current_exe()`). `--bin-path` / `--no-bin` override. If the binary fails to copy and `--no-bin` was not given, the install exits non-zero.
- Idempotent with data-safety: an existing destination is only replaced when it contains `SKILL.md` (a prior install marker); otherwise the install bails. Replacement is atomic-ish: copy to a sibling temp dir, rename the old install aside, swap into place. `--dry-run` prints the plan without writing.
- On Windows, the install target prefers `USERPROFILE` (Git Bash/CI shells set `HOME` to POSIX-style paths that Rust resolves to a bogus drive-root location).
- `node_platform_key()` in `install.rs` maps rustc target consts → node platform/arch naming; the launcher resolves the platform binary via the npm optionalDependency `@trad-skill/<key>` (the old strategy-1 local-binary lookup was removed — the tarball's `bin/` only ships the JS launcher). Keep the key and the 5 npm package names in sync (unit-tested).
- The `files` field in `package.json` controls what bunx/npx packs: `bin/`, `skills/`, and the doc/LICENSE files.

## Source repos (read-only reference)

- `../TradingAgents` — TauricResearch original (pipeline, prompts, dataflows)
- `../TradingAgents-CN` — China market fork (A股/港股 analysts, Tushare/AKShare)

Prompts and methodology are distilled from these. When updating prompts, re-extract verbatim from source — never rewrite from memory.

## Coding Standards

### Rust
- All code must pass `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` before committing.
- Use `anyhow` for error handling. No `unwrap()`/`expect()` in non-test code.
- All HTTP clients must use `rustls-tls`. Never introduce `native-tls` or `openssl` (breaks cross-compilation).
- No `cfg(target_os)` platform-specific code unless covering all 7 build targets.
- No C-dependent system crates (e.g. `openssl-sys`, `sqlite3-sys`).

### Cross-compilation compatibility
New dependencies must support all 7 CI targets:
- linux-{gnu,musl} x {x86_64,aarch64}
- apple-darwin aarch64
- windows-msvc x {x86_64,aarch64}

### Build targets (7)
x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-unknown-linux-musl, aarch64-unknown-linux-musl, aarch64-apple-darwin, x86_64-pc-windows-msvc, aarch64-pc-windows-msvc.

When adding/removing targets, update: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `bin/trad-skill.js`, `package.json` optionalDependencies, and `npm/<platform>/package.json`.

### Release
- Version lives in **7 files that must stay in sync** (a desync silently breaks platform package install):
  `package.json` (+ its 5 `optionalDependencies` `@trad-skill/*` pins), `crates/trad-data/Cargo.toml`, and all 5 `npm/<platform>/package.json`.
- Update `CHANGELOG.md` before release.
- Push tag `vX.Y.Z` to trigger the release workflow (`.github/workflows/release.yml`). CI builds the same 7 binaries; the `install` subcommand ships with them — no new artifacts.

## Git workflow (required for every change)

Follow this flow for any code or docs change (see [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming and Conventional Commits):

1. **Branch first.** Never commit directly to `main`. Cut a branch off `main` (`feat/<slug>`, `fix/<slug>`, `docs/<slug>`, etc.) and do all work there.
2. **Commit in batches.** Split the work into several small, logical Conventional Commits (e.g. `fix(rust): ...`, `feat(rust): ...`, `docs: ...`, `chore(release): ...`) rather than one giant commit.
3. **Keep docs in sync.** In the same change, update `AGENTS.md`, `README.md` / `README_CN.md` (kept in parity), and `CHANGELOG.md` whenever behavior, flags, or conventions change. A code change without its doc update is incomplete.
4. **Gate before push.** Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` (from `crates/trad-data/`) and confirm they pass before pushing.
5. **Push last.** Only push the branch after the local gates pass (`git push -u origin <branch>`).
6. **Open a PR for review.** Open a Pull Request against `main` and **request the user's review** before merging. Do not self-merge. Squash-merge per CONTRIBUTING.md.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the branching model (GitHub Flow), Conventional Commits conventions, PR checklist, and the release 7-file version-bump procedure.

## Gotchas

- `.omo/` is gitignored orchestration state — never commit it.
- `.codegraph/` exists for indexing — ignore it.
- `.trae/` (Trae IDE state) is gitignored and not tracked — do not commit it.
- `uv` is available; use `uv run python` for quick script checks if needed.
- Skill files live in `skills/tradingagents-analysis/` — this is the single source of truth.
- Rust source lives in `crates/trad-data/`. Run `cargo fmt`, `cargo clippy`, `cargo test` before committing.
- **`SKILL.md §2` makes the agent ask for the ticker first** when the user hasn't named one. Keep this prerequisite step.
