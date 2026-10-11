# Changelog

All notable changes to the Understrata project will be documented in this file.

## [Unreleased] - Milestone 3 (Walk the World)

- `us_render`: Multi-material terrain rendering (`split_terrain_buffers_by_material`, `ChunkRenderEntities`) with distinct `StandardMaterial`s and textures for `MAT_DIRT`, `MAT_STONE`, and `MAT_GRASS`, eliminating the single-dirt-texture limitation so stone cliffs and caves render with authentic stone textures and meadow surfaces render with lush grass.
- `assets`: Generated a comprehensive suite of seamless 16×16 pixel-art placeholder textures for stone (`stone`, `stone_cobble`, `stone_smooth`, `stone_brick`, `stone_slate`, `stone_cracked`, `stone_granite`, `stone_mossy`), dirt (`dirt`, `dirt_coarse`, `dirt_dry`, `dirt_packed`, `dirt_clay`, `dirt_mud`), and grass (`grass`); added `us:dirt` content material and item definitions (`validate-content` passes).
- `us_sim`: Pure kinematic capsule character controller (`step_character`, MOV-001) vs continuous terrain SDF; walk (4.3 m/s), sprint (6.5 m/s), crouch (2.0 m/s); walkable slope threshold (`normal.y >= cos(65°)`) with steep slope slide; step-up up to 0.6 m over obstacles.
- `us_sim`: Tuned walkable slope limit to 65° (`cos(65°) ≈ 0.4226`) and ground snap distance to 0.35 m, allowing players to navigate natural rolling hills and slopes freely without slipping; implemented active downhill tangent sliding acceleration and uphill climb velocity cancellation on very steep hills (> 65°).
- `us_sim`: Post-edit depenetration safeguard (`depenetrate_player`, MOV-002) ejecting players to the nearest free pose without teleporting > 1.5 m.
- `us_sim`: Jump physics with 100 ms coyote time and 100 ms jump buffering; low ceiling headroom probe preventing un-crouch under obstacles (MOV-003); 2,000-tick automated fuzz runner test verifying zero tunneling and no stuck > 1 s.
- `us_net`: Networking protocol crate with versioned `ClientMessage` and `ServerMessage` envelopes (`PROTOCOL_VERSION = 1`, NET-002); roundtrip serialization tests; connection handshake validator rejecting mismatched versions.
- `us_net`: Network latency and packet loss simulator (`NetworkSimulator`, NET-010) supporting simulated ping and drop rates.
- `us_game`: Single-player authoritative local server architecture (`LocalServer`, NET-001); client intent dispatch (`PlayerInputIntent`, `EditIntent`); server state reconciliation (`PlayerStateDelta`, `EditOpDelta`); test verifying SP gameplay never mutates world outside server systems.
- `us_render`: 3D low-poly faceted pickaxe viewmodel pass (`build_viewmodel_tool_mesh`, `ViewModelTool`, RND-009) with dynamic mouse look sway, walking bob, dig swing animation, and forward SDF wall pushback preventing clipping at any FOV.
- `us_render`: Toggleable 3rd-person spring-arm camera mode (`CameraPerspective`, toggled via `V` or `F5`) with continuous SDF occlusion raycasting preventing the camera from clipping outside caves or through walls.
- `us_render`: 3D low-poly faceted miner character model (`CharacterVisualRoot`, `CharacterLimbs`, `spawn_character_model`) with safety helmet, glowing cyan headlamp, denim work pants, leather boots, and equipped pickaxe; articulated walk-cycle animation and dynamic dig swing animations in 3rd person.
- `us_sim`: Cave and obstacle collision overhaul: replaced naive ledge step-up with ceiling headroom and downward ledge raycast checks to prevent wall jitter/climbing; replaced omnidirectional ground probe with strict downward raycasting to eliminate wall-induced ground snaps; added ceiling impact impulse cancellation (`test_mov_001_cave_ceiling_and_wall_collision`).
- `us_audio`: Material-driven footstep system (`FootstepTracker`, AUD-002) emitting `FootstepEvent`s at cadence intervals with material pitch/volume profiles (Grass, Dirt, Stone, Sand).
- `understrata`: Added `--net-lag-ms <u32>` and `--net-loss-pct <f32>` CLI parameters to configure network simulation in developer builds (NET-010).

## Milestone 2 (Dig & Fill)

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
