# Understrata Architecture

## 1. High-Level Model

Understrata follows an **authoritative simulation + presentation shell** model:
- The game simulation is completely independent of the rendering engine.
- Single-player runs a local authoritative server in-process connected over an in-memory channel.
- Mutations flow as `Intent -> Server System -> Authoritative State -> Replicated Delta`.

```
┌──────────────── CLIENT ────────────────┐        ┌──────────── SERVER (authoritative) ───────────┐
│ Input → Intent msgs ─────────────────────net───▶│ Validates intents → mutates World state       │
│ Prediction (player move, dig preview)  │        │  • Terrain/Block edit ops (§5.6)              │
│ Chunk mesh builder (async)             │◀───────│  • Entities (mobs, items, projectiles)        │
│ Render, UI, Audio, Particles           │ deltas │  • Fluids, light, AI, spawning, worldgen      │
└────────────────────────────────────────┘        └───────────────────────────────────────────────┘
```

## 2. Workspace Layout

- `crates/us_core`: Pure core library (IDs, coordinates, math, deterministic RNG, gradient noise, fixed tick constants). **NO Bevy**.
- `crates/us_content`: Data schemas for items, blocks, materials, recipes, loaders, and content registry. **NO Bevy**.
- `crates/us_voxel`: Chunk storage (SDF + materials), block layers, edit operations, raycasts. **NO Bevy**.
- `crates/us_mesh`: Surface Nets terrain mesher, block mesher, flat shading, AO bake. **NO Bevy**.
- `crates/us_worldgen`: Deterministic chunk generation, biomes, cave carving, ore distribution. **NO Bevy**.
- `crates/us_sim`: Simulation systems (lighting propagation, cellular-automata fluids, navigation graph). **NO Bevy**.
- `crates/us_net`: Networking protocol, packet serialization, op replication. **NO Bevy**.
- `crates/us_game`: Bevy gameplay plugins, ECS systems, player movement, inventory, combat.
- `crates/us_render`: Bevy rendering, custom shaders (triplanar, grid overlay), post-processing, particles.
- `crates/us_ui`: Bevy UI for HUD, menus, mode toasts, minimap.
- `crates/us_audio`: Bevy audio mixer, positional sound, reverb.
- `crates/us_tools`: CLI utilities (`validate-content`, `dump-chunk`, `bake-atlas`).
- `crates/understrata`: The main executable (supports `--headless`, `--server`, and standard client).

## 3. Strict Rules

1. Engine-agnostic core crates must never import Bevy.
2. Content is 100% data-driven in `assets/data/` using RON format.
3. World generation is bit-exact and deterministic across platforms using `libm` and integer hashing.
4. Hot paths must avoid per-frame heap allocations.
