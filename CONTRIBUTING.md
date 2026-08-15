# Contributing to mania-converter

Thanks for your interest in contributing! This document explains the workflow, rules, and quality standards for this repository. It applies to **everyone, including the maintainer** — all changes enter the codebase through pull requests.

Licensed under [Apache-2.0](LICENSE). By contributing, you agree that your work is licensed under the same terms.

---

## 1. Branching model

| Branch | Purpose | Receives PRs from |
|---|---|---|
| `main` | Stable trunk; always equals the latest released version | `develop/v_X_Y` (release PRs), `hotfix/*` |
| `develop/v_X_Y` | Development branch for version `X.Y` (e.g. `develop/v_0_6`); disposable after release | `feature/*`, `fix/*`, `refactor/*`, `docs/*`, `chore/*`, forks |
| `hotfix/*` | Emergency fixes for the latest release | — |

```
main ────────────────────────────────●─── (tag v0.6.0, release)
        ▲  release PR                 │
develop/v_0_6 ──●──●──●───────────────┘ (archived after release)
        ▲  daily PRs (squash)
feature/xxx ──┘
```

- The default branch is `main`. New version branches are cut from `main`.
- A version branch is **abandoned** once it is merged into `main`; the next version starts a fresh `develop/v_X_Y`.
- Hotfixes go straight to `main`. If a hotfix also affects the branch under development, cherry-pick it there.
- Tags `vX.Y.Z` are only ever placed on `main`.

## 2. Development setup

- Rust stable **1.85+** (edition 2024).
- Build and test:

```sh
cargo build --all-features
cargo test  --all-features
```

## 3. How to contribute (everyone, maintainer included)

1. Fork the repository (external contributors) or create a branch in the repository (maintainer), named `feature/...`, `fix/...`, `refactor/...`, `docs/...` or `chore/...` as appropriate.
2. Commit using [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `test:`, `perf:` … This feeds the automated changelog.
3. Push and open a pull request to the correct target:
   - day-to-day work → the current development branch (`develop/v_X_Y`);
   - emergency fix for a released version → `main` (branch name `hotfix/...`).
4. The CI bot automatically runs the quality gates (see §4) and AI review leaves comments on the PR.
5. Address all failing checks and review comments, then merge.

**There is no direct push to `main` or `develop/v_*` — branch protection rulesets apply to all roles, including the repository owner.** There is no human approval requirement (you cannot approve your own PR); the required status checks are the gate. Merging happens via the GitHub UI button:

- everyday PRs → **squash merge**;
- the release PR `develop/v_X_Y` → `main` → **merge commit**.

## 4. Quality gates (enforced by the CI bot)

Every PR and every push must pass:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
```

Run them locally before pushing so the bot stays happy. Additional checks (feature matrix, `cargo semver-checks`, `cargo audit`) are enabled as the project grows.

## 5. Code style

- `cargo fmt` output is the only accepted formatting; clippy warnings are errors.
- This project is primarily a **library**: every public item should have a doc comment, and `///` examples are compiled as doctests — keep them valid.
- Error handling: the library uses `thiserror` error types on its public API; `anyhow` belongs to binaries only.

## 6. Testing guidelines

- New functionality must come with tests; bug fixes should come with a regression test.
- Put small, hand-made fixtures in `tests/fixtures/`. Do **not** commit large beatmap packs or media files (music, images) — keep the repository light.
- Prefer round-trip tests (`osu → mc → osu`) and snapshot tests for serialization output.
- Malformed-input and edge-case tests (timing, BOM/CRLF, zip path traversal) are especially valued.

## 7. Release process (automated)

1. When `develop/v_X_Y` is feature-complete, open the release PR: `develop/v_X_Y` → `main`.
2. After the checks pass and the PR is merged, the bot:
   - tags the merge commit `vX.Y.Z` (version taken from `Cargo.toml`);
   - builds binaries for Windows / macOS / Linux;
   - generates the changelog (AI-assisted release notes);
   - creates the GitHub Release;
   - publishes the crate to crates.io.
3. The merged version branch is archived; development of the next version starts from `main`.

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
| `main` | 稳定主干；永远等于最新已发布版本 | `develop/v_X_Y`（发布 PR）、`hotfix/*` |
| `develop/v_X_Y` | 版本 `X.Y` 的开发分支（如 `develop/v_0_6`）；发布后即废弃 | `feature/*`、`fix/*`、`refactor/*`、`docs/*`、`chore/*`、外部 fork |
| `hotfix/*` | 对已发布版本的紧急修复 | — |

```
main ────────────────────────────────●─── (打 tag v0.6.0，发布)
        ▲  发布 PR                    │
develop/v_0_6 ──●──●──●───────────────┘ （发布后归档）
        ▲  日常 PR（squash）
feature/xxx ──┘
```

- 默认分支是 `main`，新版本分支从 `main` 切出。
- 版本分支合入 `main` 后即**废弃**，下个版本重新开一条 `develop/v_X_Y`。
- hotfix 直接进 `main`；若正在开发的版本分支也受该问题影响，请 cherry-pick 过去。
- 标签 `vX.Y.Z` 只打在 `main` 上。

## 2. 开发环境

- Rust stable **1.85+**（edition 2024）。
- 构建与测试：

```sh
cargo build --all-features
cargo test  --all-features
```

## 3. 贡献流程（所有人适用，包括维护者）

1. fork 本仓库（外部贡献者）或在仓库内新建分支（维护者），按用途命名：`feature/...`、`fix/...`、`refactor/...`、`docs/...`、`chore/...`。
2. 按 [Conventional Commits](https://www.conventionalcommits.org/) 规范提交（`feat:`、`fix:`、`refactor:`、`docs:`、`chore:`、`test:`、`perf:` 等）。自动 changelog 依赖这一规范。
3. push 并开 Pull Request，目标分支：
   - 日常工作 → 当前开发分支（`develop/v_X_Y`）；
   - 已发布版本的紧急修复 → `main`（分支名 `hotfix/...`）。
4. CI 机器人自动运行质量门禁（见第 4 节），AI 评审会在 PR 上留下评论。
5. 修复所有不通过的检查和评审意见，然后合并。

**禁止直接 push 到 `main` 或 `develop/v_*`——分支保护规则对所有角色生效，包括仓库 owner。** 本项目不设人工审批（你无法批准自己的 PR），必过的状态检查就是审核门禁。合并在 GitHub 网页上点击按钮完成：

- 日常 PR → **squash merge**；
- 发布 PR（`develop/v_X_Y` → `main`）→ **merge commit**。

## 4. 质量门禁（由 CI 机器人强制执行）

每个 PR 和每次 push 都必须通过：

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
```

push 前请在本地先跑一遍。随着项目发展，还会启用更多检查（feature 组合矩阵、`cargo semver-checks`、`cargo audit`）。

## 5. 代码风格

- 唯一接受的格式是 `cargo fmt` 的输出；clippy 警告视为错误。
- 本项目主要作为**库**使用：所有公共条目都应写文档注释，`///` 中的示例会作为 doctest 编译执行，请保持其正确。
- 错误处理：库的公共 API 使用 `thiserror` 定义的错误类型；`anyhow` 只用于 binaries。

## 6. 测试规范

- 新功能必须附带测试；修 bug 应附带回归测试。
- 小型手工构造的测试素材放在 `tests/fixtures/`。**不要**提交大型谱面包或媒体文件（音频、图片），保持仓库轻量。
- 优先写往返测试（`osu → mc → osu`）和序列化输出的快照测试。
- 特别欢迎异常输入与边界用例的测试（时序、BOM/CRLF、zip 路径穿越等）。

## 7. 发布流程（全自动）

1. `develop/v_X_Y` 功能齐备后，开发布 PR：`develop/v_X_Y` → `main`。
2. 检查全部通过且 PR 合并后，机器人自动：
   - 在合并提交上打 tag `vX.Y.Z`（版本号取自 `Cargo.toml`）；
   - 构建 Windows / macOS / Linux 三平台二进制；
   - 生成 changelog（AI 辅助撰写发布说明）；
   - 创建 GitHub Release；
   - 发布 crate 到 crates.io。
3. 已合并的版本分支归档；下一版本的开发从 `main` 重新开始。

## 8. 有问题？

开 issue，或直接联系维护者。
