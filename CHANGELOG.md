# Changelog

All notable changes to the Understrata project will be documented in this file.

## [Unreleased] - Milestone 2 (Dig & Fill)

### Added
- `us_voxel`: Authoritative, deterministic `EditOp` system (`apply_edit_op`) using 1/16 m fixed-point coordinates (TER-004, TER-005); bit-exact op replay logging (`LoggedOp`, TER-009); continuous SDF terrain raycasting (`raycast_terrain`); indestructible bedrock protection (TER-012).
- `us_render`: Interactive digging and filling (`terrain_interaction_system`) via Left Click (Dig / Subtract) and `F` (Fill / Add); dynamic brush sizing (keys 1/2/3 for 1.0 m, 1.8 m, 3.0 m); zero-flicker in-place remeshing (`*mesh_asset = new_mesh`, TER-007).
- `us_render`: Material-colored debris cube particle physics system (`DebrisParticle`, TER-019) with randomized velocity, gravity, and scaling lifetime.
- `us_render`: Fractional yield accumulator (`YieldAccumulator`, TER-008) tracking extracted volume per material and awarding discrete drops.
- `us_render`: Minimalist HUD overlay with center reticle (`+`), action key legend, yield counter, and toggleable F3 debug overlay (`DebugOverlayState`, DEV-002).

## Milestone 1 (Terrain on Screen)

### Added
- `us_voxel`: `Field<T>` with zero-allocation `Uniform(T)` and `Dense(Box<[T]>)` storage; `TerrainChunk` (TER-001) under 64 B when uniform; quantized SDF (`SDF_QUANT = CELL / 32.0`); 34³ `PaddedChunkArray` for seamless multi-chunk meshing snapshots; continuous trilinear sampling and gradient computation in `us_voxel::query`.
- `us_mesh`: Naive Surface Nets terrain mesher (`mesh_terrain`) outputting unindexed triangles with flat face normals (TER-002); surface skin procedural overlay (TER-011) converting up-facing dirt triangles (`normal.y > 0.7`) to grass.
- `us_worldgen`: Deterministic test chunk generator (`gen_test_chunk`) combining 2D rolling hills with 3D cave carving using `us_core` deterministic noise.
- `us_tools`: `worldgen-preview` CLI subcommand to generate and report statistics on chunk meshes.
- `us_render`: Mesh buffer converter to Bevy 3D `Mesh` with vertex colors; `FlyCamera` controller (MOV-011) supporting WASD flight and mouse look; directional sunlight with shadow maps and ambient sky lighting; chunk spawning pipeline (RND-001).
- `docs/DECISIONS.md`: Recorded ADR-0003 for the 1 m Surface Nets faceted mesher strategy (D-01).


## Milestone 0 (Foundations)

### Added
- Workspace and crate layout with 13 modular crates separating pure engine-agnostic logic from presentation shell.
- Toolchain pinned to Rust 1.97.0 and Bevy pinned to 0.19.1.
- `us_core`: Coordinate systems (`WorldPos`, `CellPos`, `ChunkPos`), Euclidean spatial conversions, world constants, SplitMix64 deterministic RNG, integer coordinate hashing, and pure `libm` gradient noise / fBm.
- `us_content`: RON schemas for `MaterialDef`, `BlockDef`, `ItemDef`, content registry, cross-reference validator, and initial data definitions for stone and stone bricks.
- `us_tools`: Developer CLI with `validate-content` command.
- `understrata`: Headless test harness CLI supporting `--headless`, `--frames`, `--ticks`, `--seconds`, structured JSON logging, and clean exit codes.
- Architectural documentation and records: `ARCHITECTURE.md`, `DECISIONS.md`, `OPEN_QUESTIONS.md`, `DEPENDENCIES.md`, `ART_BIBLE.md`, `ASSET_REQUESTS.md`.
- CI workflow for automated testing, fmt, clippy, and headless smoke runs (`.github/workflows/ci.yml`).
