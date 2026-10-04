# Contributing to mania-converter

Thanks for your interest in contributing! This document explains the workflow, rules, and quality standards for this repository. It applies to **everyone, including the maintainer** — all changes enter the codebase through pull requests.

Licensed under [Apache-2.0](LICENSE). By contributing, you agree that your work is licensed under the same terms.

---

## 1. Branching model

| Branch | Purpose | Receives PRs from |
|---|---|---|
| `main` | Integration trunk; not necessarily the latest released version | `feature/*`, `fix/*`, `refactor/*`, `docs/*`, `chore/*`, `hotfix/*`, forks |
| `release/X.Y.Z` | Release candidate, cut from a tested `main` commit | Created by the maintainer; pushing it triggers the release workflow |

```
feature/xxx ──●──●──┐
                    ├─ squash PR ─> main ──●── release/0.6.2 ──> tag v0.6.2
hotfix/xxx  ──●─────┘
```

- The default branch is `main`. All development PRs target it.
- `release/*` branches are not merged back. They exist to trigger publishing for one version.
- Tags `vX.Y.Z` are placed on `main` history, at the released commit.
- Branches named `develop/v_X_Y` are retired; the next version no longer opens one.

## 2. Development setup

- Rust stable **1.85+** (edition 2024).
- Build and test:

```sh
cargo build --all-features
cargo test  --workspace --all-features
```

- Local git hooks install themselves automatically via `cargo-husky` (a dev-dependency) the first time you run `cargo test` — no manual setup. They run on every commit, see §4.

## 3. How to contribute (everyone, maintainer included)

1. Fork the repository (external contributors) or create a branch in the repository (maintainer), named `feature/...`, `fix/...`, `refactor/...`, `docs/...`, `chore/...` or `hotfix/...` as appropriate.
2. Commit using [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `test:`, `perf:` … This feeds the automated changelog. Add a component scope such as `fix(lib):` or `feat(standalone):`, since one changelog covers the library and the bundled applications.
3. Push and open a pull request against `main`, including emergency hotfixes.
4. The CI bot automatically runs the quality gates (see §4) and AI review leaves comments on the PR.
5. Address all failing checks and review comments, then merge.

**There is no direct push to `main` — branch protection rulesets apply to all roles, including the repository owner.** There is no human approval requirement (you cannot approve your own PR); the required status checks are the gate. Everyday PRs are merged with **squash merge**, and the source branch is deleted.

## 4. Quality gates (enforced by the CI bot)

Every PR and every push must pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```

### Local pre-commit hook

A pre-commit hook is installed automatically by `cargo-husky` (a dev-dependency) the first time you run `cargo test` — nothing to install by hand. On every `git commit` it runs and reports each step with `[PASS]` / `[FAIL]`:

1. `cargo fmt --all` — formatting is fixed in the working tree only;
2. `cargo clippy --workspace --fix` — machine-applicable warnings are fixed in the working tree only;
3. `cargo fmt --all` — re-format after the fixes;
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — remaining warnings abort the commit.

The hook never stages files or changes the index. Automatic fixes remain in the working tree; review and explicitly stage the intended changes before retrying a commit. All four steps run even if an earlier step fails or changes tracked files. After all steps finish, any failure or newly produced fix aborts the commit, so formatting and Clippy fixes can be reviewed and staged together. It compares the working tree before and after each fixer, so unchanged pre-existing edits alone do not cause an abort. Checks run against the working tree, not an isolated copy of the staged snapshot.

When a step fails, the commit is cancelled: fix the issue, review and stage only the intended files, then retry `git commit`. In a hurry, `git commit --no-verify` skips the hook — CI still runs the gates above and remains the source of truth.

The hook scripts live in `.cargo-husky/hooks/`. `cargo test` runs in CI (too slow for every commit). Additional checks (feature matrix, `cargo semver-checks`, `cargo audit`) are enabled as the project grows.

## 5. Code style

- `cargo fmt` output is the only accepted formatting; clippy warnings are errors.
- This project is primarily a **library**: every public item should have a doc comment, and `///` examples are compiled as doctests — keep them valid.
- Error handling: the public API uses `std::io::Result`; this repository declares no `anyhow` dependency.

## 6. Testing guidelines

- New functionality must come with tests; bug fixes should come with a regression test.
- Put small, hand-made fixtures in `tests/fixtures/`. Do **not** commit large beatmap packs or media files (music, images) — keep the repository light.
- Prefer round-trip tests (`osu → mc → osu`) and snapshot tests for serialization output.
- Malformed-input and edge-case tests (timing, BOM/CRLF, zip path traversal) are valued.

## 7. Release process (automated)

1. Run `bash ci/bump-lib X.Y.Z` on a clean tree. It creates the `bump/X.Y.Z` branch, writes the version into the root `Cargo.toml`, refreshes `Cargo.lock`, prepends the `git-cliff` changelog entry, and commits everything as `bump(lib): <old> -> <new>`. It refuses to run when the argument is not `X.Y.Z`, the tree is dirty, the version is unchanged, or the tag already exists. The changelog entry is never written by hand.
2. Push that branch and open a PR to `main`. Merge it once `ci` passes. `CHANGELOG.md` and the version then agree on `main`, so there is nothing to sync afterwards.
3. Cut `release/X.Y.Z` from that merged commit and push it. The branch version must match `Cargo.toml`; version `0.6.2` means the branch `release/0.6.2`. The branch carries no commits of its own.
4. [`.github/workflows/release.yml`](.github/workflows/release.yml) runs the quality gates on the pushed commit and publishes the GitHub Release `vX.Y.Z`, using the `## vX.Y.Z` section of `CHANGELOG.md` as its notes. A missing, empty or duplicated section only reports that it is waiting.
5. The same Release carries two Windows x64 executables built from that commit: `mania-converter-standalone-vX.Y.Z.exe` and `mania-converter-webapp-vX.Y.Z.exe`. A version that already has a tag or Release is kept as-is — a retry only re-attaches the executables — so bump the version again instead of reusing the branch. Nothing needs merging back: the release branch is a copy of `main` and may be deleted after publishing. crates.io publishing returns later.

```sh
# 1) version, lockfile and changelog together, through a PR to main
bash ci/bump-lib 0.6.2
git push -u origin bump/0.6.2

# 2) after the PR is merged, release that commit
git switch main && git pull --ff-only origin main
git switch -c release/0.6.2
git push -u origin release/0.6.2
```

`ci/bump-lib` needs a local `git-cliff` (`cargo install git-cliff --locked`) and calls `git commit`, so the pre-commit hook in §4 runs as usual.

The repository must allow Actions to write contents (Settings → Actions → General → Workflow permissions).

## 8. Questions?

Open an issue, or contact the maintainer.

---

# 参与 mania-converter 开发（中文）

欢迎贡献！本文件说明本仓库的协作流程、规则与质量标准。它适用于**包括维护者在内的所有人**——所有改动都必须通过 Pull Request 进入代码库。

本项目采用 [Apache-2.0](LICENSE) 许可。参与贡献即表示你同意你的成果按同样条款授权。

---

## 1. 分支模型

| 分支 | 用途 | 接受的 PR 来源 |
|---|---|---|
| `main` | 集成主干；不要求等于最新已发布版本 | `feature/*`、`fix/*`、`refactor/*`、`docs/*`、`chore/*`、`hotfix/*`、外部 fork |
| `release/X.Y.Z` | 发布候选分支，从已测试的 `main` 提交切出 | 维护者创建；推送即触发发布流程 |

```
feature/xxx ──●──●──┐
                    ├─ squash PR ─> main ──●── release/0.6.2 ──> tag v0.6.2
hotfix/xxx  ──●─────┘
```

- 默认分支是 `main`，所有开发 PR 都指向它。
- `release/*` 分支不合并回主干，它的作用是触发某个版本的发布。
- 标签 `vX.Y.Z` 打在 `main` 的历史提交上，即实际发布的那个提交。
- `develop/v_X_Y` 分支已停用，下个版本不再新开。

## 2. 开发环境

- Rust stable **1.85+**（edition 2024）。
- 构建与测试：

```sh
cargo build --all-features
cargo test  --workspace --all-features
```

- 本地 git 钩子通过 `cargo-husky`（dev-dependency）自动安装：首次运行 `cargo test` 时即装好，无需手动配置。每次 commit 都会运行，见第 4 节。

## 3. 贡献流程（所有人适用，包括维护者）

1. fork 本仓库（外部贡献者）或在仓库内新建分支（维护者），按用途命名：`feature/...`、`fix/...`、`refactor/...`、`docs/...`、`chore/...`、`hotfix/...`。
2. 按 [Conventional Commits](https://www.conventionalcommits.org/) 规范提交（`feat:`、`fix:`、`refactor:`、`docs:`、`chore:`、`test:`、`perf:` 等）。自动 changelog 依赖这一规范；请带上组件 scope（如 `fix(lib):`、`feat(standalone):`），因为一份 changelog 同时覆盖库和随附的应用。
3. push 并向 `main` 开 Pull Request，紧急修复同样走 `main`。
4. CI 机器人自动运行质量门禁（见第 4 节），AI 评审会在 PR 上留下评论。
5. 修复所有不通过的检查和评审意见，然后合并。

**禁止直接 push 到 `main`——分支保护规则对所有角色生效，包括仓库 owner。** 本项目不设人工审批（你无法批准自己的 PR），必过的状态检查就是审核门禁。日常 PR 使用 **squash merge**，合并后删除源分支。

## 4. 质量门禁（由 CI 机器人强制执行）

每个 PR 和每次 push 都必须通过：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```

### 本地 pre-commit 钩子

本地钩子由 `cargo-husky`（dev-dependency）自动安装：首次运行 `cargo test` 时即装好，无需手动配置。每次 `git commit` 都会运行并逐项报告 `[PASS]` / `[FAIL]`：

1. `cargo fmt --all` — 自动修复工作区中的格式，不暂存；
2. `cargo clippy --workspace --fix` — 自动修复工作区中机器可修的警告，不暂存；
3. `cargo fmt --all` — 修复后重新格式化；
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — 剩余警告将中止 commit。

钩子不会暂存文件或修改暂存区。自动修复保留在工作区中；请检查并手动暂存需要提交的改动，再重新提交。即使前面的步骤失败或改变了已跟踪文件，四个步骤仍会全部执行。全部结束后，只要任一步失败或产生了新修复，就统一中止提交，便于一次检查并暂存格式与 Clippy 修复。钩子比较每一步修复前后的工作区，因此原本就存在、且未被修复改变的修改不会单独导致提交中止。检查针对工作区运行，并非针对暂存区的隔离副本。

任一步失败 commit 都会被取消：修复后检查并仅暂存需要提交的文件，再重新 `git commit`。赶时间可用 `git commit --no-verify` 跳过钩子——CI 仍会执行上面的门禁，CI 才是最终裁判。

钩子脚本位于 `.cargo-husky/hooks/`。`cargo test` 在 CI 中运行（对每次 commit 来说太慢）。随着项目发展，还会启用更多检查（feature 组合矩阵、`cargo semver-checks`、`cargo audit`）。

## 5. 代码风格

- 唯一接受的格式是 `cargo fmt` 的输出；clippy 警告视为错误。
- 本项目主要作为**库**使用：所有公共条目都应写文档注释，`///` 中的示例会作为 doctest 编译执行，请保持其正确。
- 错误处理：公共 API 使用 `std::io::Result`；本仓库不声明 `anyhow` 依赖。

## 6. 测试规范

- 新功能必须附带测试；修 bug 应附带回归测试。
- 小型手工构造的测试素材放在 `tests/fixtures/`。**不要**提交大型谱面包或媒体文件（音频、图片），保持仓库轻量。
- 优先写往返测试（`osu → mc → osu`）和序列化输出的快照测试。
- 特别欢迎异常输入与边界用例的测试（时序、BOM/CRLF、zip 路径穿越等）。

## 7. 发布流程（全自动）

1. 在干净的工作区运行 `bash ci/bump-lib X.Y.Z`。它会创建 `bump/X.Y.Z` 分支，把版本号写入根目录 `Cargo.toml`，刷新 `Cargo.lock`，用 `git-cliff` 追加 changelog 条目，并以 `bump(lib): <旧> -> <新>` 提交。参数不是 `X.Y.Z`、工作区不干净、版本未变化或 tag 已存在时都会拒绝执行。changelog 条目不要手工编写。
2. 推送该分支并向 `main` 开 PR，`ci` 通过后合并。此时 `CHANGELOG.md` 与版本号已经在 `main` 上一致，之后不需要任何同步。
3. 从该合并提交切出 `release/X.Y.Z` 并推送。分支版本必须与 `Cargo.toml` 一致：版本 `0.6.2` 对应分支 `release/0.6.2`。该分支本身不含任何提交。
4. [`.github/workflows/release.yml`](.github/workflows/release.yml) 对推送的提交运行质量门禁，并发布 GitHub Release `vX.Y.Z`，发布说明取 `CHANGELOG.md` 中的 `## vX.Y.Z` 小节。小节缺失、为空或重复时只提示正在等待。
5. 同一个 Release 会附带由该提交构建的两个 Windows x64 可执行文件：`mania-converter-standalone-vX.Y.Z.exe` 和 `mania-converter-webapp-vX.Y.Z.exe`。该版本已有 tag 或 Release 时保持原样（重跑只会重新附上可执行文件），需要再次发布就递增版本号，而不是复用同一分支。不需要合并回主干——发布分支只是 `main` 的副本，发布完可以删除。crates.io 发布稍后再补。

```sh
# 1) 版本号、lockfile、changelog 一起走 PR 进 main
bash ci/bump-lib 0.6.2
git push -u origin bump/0.6.2

# 2) PR 合并后，发布该提交
git switch main && git pull --ff-only origin main
git switch -c release/0.6.2
git push -u origin release/0.6.2
```

`ci/bump-lib` 需要本地安装 `git-cliff`（`cargo install git-cliff --locked`），并且内部会执行 `git commit`，因此第 4 节的 pre-commit 钩子照常运行。

仓库需允许 Actions 写入 contents（Settings → Actions → General → Workflow permissions）。

## 8. 有问题？

开 issue，或直接联系维护者。
