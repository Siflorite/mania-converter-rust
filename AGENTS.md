# Project guide for AI agents

## Purpose and layout

`mania-converter` is a Rust library for Malody and osu!mania beatmaps, with a standalone CLI and an Actix Web upload service. The root crate is version 0.6.0, edition 2024; the two binary crates use edition 2021 and have independent versions. Use the current stable Rust toolchain with rustfmt and Clippy.

- `src/lib.rs`: public modules and shared `BeatMapInfo`; crate documentation includes `README.md`.
- `src/malody.rs`: serde MC data model, beat arithmetic, MC parsing/writing and conversion to osu data. `src/malody/mcz2osz.rs` handles MCZ archive conversion and batch processing.
- `src/osu.rs`: osu parsing/writing, legacy/v128 hit objects, timing, metadata and conversion to MC data. `src/osu/calc_sr.rs` calculates star ratings; `src/osu/osz_func.rs` processes OSZ archives. Check module declarations before assuming a source file is active.
- `src/misc.rs`: shared internal helpers, including filename handling.
- `src/graphx.rs` and `src/graphx/info_generation.rs`: beatmap information cards, using Handlebars SVG templates and resvg rendering.
- `binaries/standalone`: interactive CLI for MCZ conversion or OSZ info cards in the working directory.
- `binaries/webapp`: Actix Web upload/conversion/download service, currently binding to `0.0.0.0:80`. `shuttle_main.rs` is an alternative entrypoint, not the default binary.
- `svg/` and `font/`: runtime rendering assets, resolved relative to the working directory. Run rendering and its tests from the repository root.
- `tests/`: integration tests for conversion, star ratings and rendering; existing fixtures are in `tests/beatmaps/`. Some tests generate files next to fixtures; inspect the diff after running tests.

## Development and validation

Run from the repository root:

```sh
cargo build --workspace --all-features
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```

Run the CLI with `cargo run -p mania-converter-standalone`, or the web service with `cargo run -p mania-converter-webapp`. The latter starts a listening server; only run it when needed. Target individual integration tests with `cargo test --test osu2mc`, `--test old_mc`, `--test sr`, or `--test render`.

`.github/workflows/ci.yml` defines the CI gates (fmt, Clippy, tests and docs). CI disables cargo-husky installation via `CARGO_HUSKY_DONT_INSTALL_HOOKS=true`. Local cargo-husky installs `.cargo-husky/hooks/pre-commit`; it runs fmt, Clippy auto-fixes, fmt again and a strict Clippy gate. Fixes stay in the working tree: hooks and automated checks must never stage files or otherwise modify the index. Each fixer compares tracked-file contents before and after execution and aborts the commit if it produces changes, even when Cargo succeeds. Unchanged pre-existing edits alone do not abort the commit. These checks inspect the working tree, not an isolated staged snapshot. Review fixes and explicitly stage only intended files when preparing an authorized commit.

## Change discipline

- Read `CONTRIBUTING.md` for contribution policy. Use Conventional Commits and open daily-work PRs against `develop/v_0_6`; do not push directly to protected development or main branches.
- Check `git status` before edits. Preserve unrelated work and keep commits scoped; use an isolated worktree when appropriate.
- Follow existing formatting, document public APIs, and add meaningful regression tests for behavior changes. Keep fixtures small; put new hand-made fixtures in `tests/fixtures/`. Do not add large beatmap packs, generated cards or media without a specific need.
- Preserve timing, long-note, hitsound, encoding and archive-path behavior when touching conversion. Keep optional star-rating calculation optional.
- Inspect actual signatures, manifests and workflow files: parts of README/CONTRIBUTING describe older APIs or intended future automation. Current library APIs largely return `std::io::Result`; `anyhow` is also present in the root dependencies, despite the contribution guide's aspirational error-type policy. Do not introduce an unrelated API/error-type migration.
- Do not assume OSZ-to-MCZ archive conversion or release automation is complete merely because it appears in a roadmap or documentation.
