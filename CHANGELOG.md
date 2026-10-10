# Changelog

All notable changes to the Understrata project will be documented in this file.

## [Unreleased] - Milestone 0 (Foundations)

### Added
- Workspace and crate layout with 13 modular crates separating pure engine-agnostic logic from presentation shell.
- Toolchain pinned to Rust 1.97.0 and Bevy pinned to 0.19.1.
- `us_core`: Coordinate systems (`WorldPos`, `CellPos`, `ChunkPos`), Euclidean spatial conversions, world constants, SplitMix64 deterministic RNG, integer coordinate hashing, and pure `libm` gradient noise / fBm.
- `us_content`: RON schemas for `MaterialDef`, `BlockDef`, `ItemDef`, content registry, cross-reference validator, and initial data definitions for stone and stone bricks.
- `us_tools`: Developer CLI with `validate-content` command.
- `understrata`: Headless test harness CLI supporting `--headless`, `--frames`, `--ticks`, `--seconds`, structured JSON logging, and clean exit codes.
- Architectural documentation and records: `ARCHITECTURE.md`, `DECISIONS.md`, `OPEN_QUESTIONS.md`, `DEPENDENCIES.md`, `ART_BIBLE.md`, `ASSET_REQUESTS.md`.
- CI workflow for automated testing, fmt, clippy, and headless smoke runs (`.github/workflows/ci.yml`).
