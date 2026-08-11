# Contributing to trad-skill

Thanks for contributing! This is an AI-agent **skill** (not an application). The skill content lives in `skills/tradingagents-analysis/`; the Rust data tool + installer lives in `crates/trad-data/`.

## Branching model (GitHub Flow)

- `main` is always releasable. Every merge to `main` should leave CI green.
- Cut a branch off `main` for every change:
  - `feat/<slug>` — new capability
  - `fix/<slug>` — bug fix
  - `docs/<slug>` — documentation only
  - `refactor/<slug>` — behavior-preserving refactor
  - `chore/<slug>` — tooling, deps, CI
  - `release/<version>` — release prep
- Open a Pull Request against `main`. Use **squash-merge** so each PR lands as one commit.
- Delete the branch after merge. Prune stale branches (the legacy `fix/roundN-bugs` branches are historical and should not be reused).

## Commit messages (Conventional Commits)

```
<type>(<scope>): <subject>

<body>
```

- **Types:** `feat | fix | docs | refactor | test | chore | ci | perf`
- **Scopes:** `rust | installer | docs | skill | ci | release | npm`
- Footer: `BREAKING CHANGE: <note>` for backwards-incompatible changes (also bump the major version).

Examples: `feat(rust): add install subcommand`, `docs: deprecate npx skills add`, `fix(installer): resolve tilde on Windows`.

## Before you commit — Rust gates

From `crates/trad-data/`:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

All three must pass. Rust standards (see [AGENTS.md](AGENTS.md)): `anyhow` for errors, no `unwrap()`/`expect()` in non-test code, `rustls-tls` only (never `native-tls`/`openssl`), no C-dependent system crates, and new dependencies must build on all 7 CI targets.

## Local verification

```bash
# Installer (Rust), no side effects:
cargo run -- install --dry-run

# Real install to a throwaway dir:
cargo run -- install --dir /tmp/trad-skill-test --no-bin

# Data tool directly (no install):
cargo run -- stock --symbol AAPL

# End-to-end via bunx (after `npm pack`):
bunx trad-skill@latest --dry-run
bunx trad-skill@latest stock --symbol AAPL
```

`npx trad-skill@latest` behaves identically if `bun` is not installed.

## Pull Request checklist

- [ ] CI is green (`cargo fmt` / `clippy` / `test` + 7-platform cross-build).
- [ ] One logical change per PR.
- [ ] English (`README.md`) and Chinese (`README_CN.md`) docs are kept in parity.
- [ ] `SKILL.md §2` (the "ask for the ticker first" prerequisite) is preserved.
- [ ] **Do not edit `CLAUDE.md`** — it is a pointer to `AGENTS.md`. Edit `AGENTS.md` instead.
- [ ] If you touched the installer, verify `node_platform_key()` (install.rs) still matches the launcher's `@trad-skill/<key>` package resolution and the 5 npm package names, and `--dry-run` works.
- [ ] If you changed the default install target or flags, update README, README_CN, SKILL.md 'For AI Agents: Installation Guide' section, and AGENTS.md together.

## Release procedure

1. Bump the version **in all 7 places** (a desync silently breaks platform package install):
   - `package.json` `version`
   - the 5 `optionalDependencies` `@trad-skill/*` pins in the same `package.json`
   - `crates/trad-data/Cargo.toml` `version`
   - each of `npm/{win32-x64,win32-arm64,darwin-arm64,linux-x64,linux-arm64}/package.json`
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md` (Added / Changed / Deprecated / Fixed).
3. Commit as `chore(release): bump to X.Y.Z`.
4. Push tag `vX.Y.Z` to trigger `.github/workflows/release.yml` (7-platform build → GitHub Release + npm publish with provenance). **Confirm before tagging** — a tag publishes to the public npm registry.

## License

Apache 2.0. By contributing you agree your contributions are licensed under the same terms.
