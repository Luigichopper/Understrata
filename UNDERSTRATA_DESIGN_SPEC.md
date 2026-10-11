# UNDERSTRATA — Full-Release Design & Implementation Specification

| | |
|---|---|
| **Doc version** | 0.1 (initial draft, derived from 3 mockup screenshots + owner brief) |
| **Primary audience** | AI coding agents implementing the game |
| **Secondary audience** | The human owner (review, approve decisions in §23) |
| **Scope** | Everything required for a complete **1.0 release**, plus tagged post-1.0 ideas |
| **Status of content** | Names of items/enemies/biomes/lore are **WORKING NAMES** unless the owner approves them |

---

## 0. Agent Quick-Start (READ THIS FIRST)

### 0.1 The game in one paragraph
Understrata is a **first-person 3D sandbox adventure** with Terraria-style progression (dig deep → find ores → craft better gear → fight bosses → unlock NPCs/world changes), set in a **fully diggable, low-poly, smooth-surfaced world** that is *not* made of cubes. On top of the smooth terrain sits a second, **crisp cube-grid building layer** (walls, floors, furniture) toggled via **Grid Mode**. The signature fantasy is: *carve organic caves and hillsides with a pickaxe, then snap clean, hand-built architecture into them.* The world is vertically stratified into themed depth layers ("strata"), which is also where the title comes from.

### 0.2 The six hardest problems (design/implement these with the most care)
1. **Hybrid terrain**: a diggable signed-distance-field (SDF) terrain meshed into low-poly facets *plus* an independent cube-block layer that interpenetrates it without seams or z-fighting (§5, §6).
2. **Edit-heavy streaming world**: world = `deterministic_generator(seed) + sparse edit deltas`. Saves, multiplayer sync, and undo all depend on this (§5.6, §17, §20).
3. **Collision and AI on deformable terrain**: no static navmesh/trimesh; everything must tolerate edits at runtime (§7, §13.4).
4. **Underground rendering**: dark caves, colored local lights, volumetric fog, and cave-aware culling at 60 FPS (§9, §19).
5. **Multiplayer-ready architecture from day one**: single-player runs a local authoritative server (§17).
6. **Scope control**: this spec is large on purpose. **Priorities (P0–P3) and milestones (M0–M15) are binding**; do not build P2/P3 features before P0/P1 are solid.

### 0.3 Reading order for an agent
1. §0 (this section), §1 (vision), §2 (mockup analysis)
2. §3 (tech decision) and §4 (architecture) — **before writing any code**
3. The system section for your current milestone (§5–§20)
4. §21 (roadmap) to find your milestone's **exit criteria**
5. §22 (testing & agent harness) — **every change must be verifiable headlessly**

### 0.4 Conventions
- **RFC-2119 keywords**: MUST / MUST NOT / SHOULD / MAY have their normal meanings.
- **Feature IDs** look like `TER-004`. Reference them in commits, PRs, tests, and code comments (`// TER-004`).
- **Priority**
  - `P0` = required for the **Vertical Slice** (internal alpha, end of M10)
  - `P1` = required for **1.0**
  - `P2` = **1.0 target**, may be cut if schedule slips
  - `P3` = post-1.0
- **Milestone** tags `M0`…`M15` map to §21.
- **Units**: 1 world unit = **1 meter**. Y is up. Right-handed. `CELL = 1.0 m` (the shared terrain/block lattice spacing) unless a decision in §23 changes it.
- **Defaults vs. tunables**: any number in this doc marked *(tunable)* MUST live in a data file (`assets/data/balance/*.ron`), never hard-coded.
- **“Mockup-derived”** means a requirement inferred from the screenshots in §2. Treat as strong intent but verify against §2.3 ambiguities.

### 0.5 Agent working agreements (MUST follow)
1. **One feature ID per commit/PR.** Message format: `TER-004: add dig brush falloff`.
2. **Do not invent lore, names, or art direction.** If you need a placeholder, prefix it `placeholder_` / `TODO_NAME` and append a line to `docs/OPEN_QUESTIONS.md`.
3. **Ambiguity protocol**: pick the documented default, implement it behind a config/feature flag where cheap, log the question in `docs/OPEN_QUESTIONS.md`, and continue. Do not block.
4. **Engine-agnostic core.** Game rules, terrain, worldgen, meshing, sim, and serialization live in crates that **do not depend on Bevy** (§4.2). Bevy-touching code is a thin shell.
5. **Everything content-like is data-driven** (items, blocks, recipes, enemies, loot, biomes, structures, strings). No item stats or recipes in Rust source.
6. **Verify APIs against pinned versions.** Bevy and its ecosystem change every release. **Never rely on memory for Bevy API shapes**; read the pinned version's docs/examples (`cargo doc`, docs.rs for the exact version, or vendored source).
7. **Every change must pass**: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the headless smoke test (§22.3).
8. **Never block the main/render thread** on chunk generation, meshing, IO, compression, or pathfinding. Use task pools / worker threads and apply results on the main thread.
9. **No per-frame heap allocation in hot paths** (meshing inner loops, per-entity update loops, light propagation).
10. **Dependencies**: record every new crate in `docs/DEPENDENCIES.md` (purpose, license, maintenance status, Bevy-version compatibility). Prefer MIT/Apache-2.0. **No GPL/AGPL.**
11. **Original content only.** Terraria, Minecraft, Valheim, Enshrouded, etc. are *genre references only* (§1.3). Do not copy names, textures, sprites, text, recipes, or exact mechanics numbers from them.
12. **Keep docs alive**: update `CHANGELOG.md`, and add an ADR entry to `docs/DECISIONS.md` for any decision that changes an architecture assumption in this spec.

---

## 1. Vision

### 1.1 Elevator pitch
*“Terraria’s dig-deep-and-get-stronger adventure, in a living low-poly 3D world where the ground itself is your clay and the blocks are your bricks.”*

### 1.2 Design pillars (use these to break ties)
| # | Pillar | Meaning in practice |
|---|---|---|
| 1 | **Dig anywhere, build anywhere** | Terrain is continuously deformable; the player is never gated by a block grid when exploring, but can opt into precision building. |
| 2 | **Depth is progression** | Each stratum has distinct look, hazards, enemies, resources, and music. Going deeper is always the “next goal.” |
| 3 | **Readable low-poly beauty** | Faceted geometry + 16-px pixel-art textures + rich colored lighting. Caves are dark but *legible* (glowing flora, torches, minimap). |
| 4 | **Juicy, tactile tools** | Every dig, swing, hit, place, and pickup has visual, audio, and haptic feedback. |
| 5 | **Respect the player’s time** | Heavy QOL: auto-stack, craft-from-chests, undo, waystones, no tedious inventory juggling. |
| 6 | **Systems are data** | Content is moddable data; the engine is generic. |

### 1.3 Reference games (what to borrow, not copy)
The owner was unsure which games Understrata resembles. Closest cousins and the *design lesson* from each:

| Game | Borrow | Do **not** borrow |
|---|---|---|
| **Terraria** | Progression ladder, boss gating, NPC housing, loot/accessory depth, event pacing, QOL culture | Names, art, exact recipes/numbers, 2D constraints |
| **Enshrouded** | Smooth voxel terrain + block building coexisting, cave exploration with lighting, fantasy tone | Its specific art/IP; full-voxel approach (we use SDF + blocks) |
| **Valheim** | Terraform tools + building + biome-gated boss progression; low-poly-ish stylized look and atmosphere | Survival-heavy hunger/cold loops (optional later) |
| **Deep Rock Galactic** | Cave readability, tool feel, flares/light as gameplay, mining juice | Class/mission structure |
| **Astroneer** | Smooth terrain deformation tool feel and brush behavior | Space theme |
| **Core Keeper** | Underground biome ladder with strong per-biome identity | Top-down 2D presentation |
| **Teardown** | Destruction as fun, chunk remesh responsiveness | Fully voxel destruction |
| **Cube World / Hytale** | Low-poly voxel-adjacent adventure aesthetic | Class systems, IP |

### 1.4 Non-goals (explicitly out of scope for 1.0)
- Fully voxel (all-cube) terrain. Terrain is **SDF-smooth**; only the *building layer* is cubic.
- Realistic physics simulation (rigid-body destruction, soft bodies, structural collapse). See §7.5 for the limited physics we do ship.
- Procedural quests/story generation. Quests are hand-authored data.
- Console ports. (Windows + Linux first; macOS/Steam Deck are P2.)
- Photorealistic rendering, ray tracing, or per-pixel GI.
- MMO-scale multiplayer. Target is **2–8 player co-op** with optional dedicated server.
- Voxel-resolution fluid simulation with pressure. Fluids are cellular-automata level-based (§10).

### 1.5 Target experience / session loop
- **Minute loop** (0–5 min): swing pickaxe, carve a tunnel, hit an ore vein, craft a torch, fight a swarm, loot a chest.
- **Hour loop**: descend a stratum, build a base with a workbench + house, recruit an NPC, craft next-tier gear, prepare for a boss.
- **Campaign loop** (20–40 h for 1.0 completion *(tunable target)*): clear 5 strata, defeat 5 bosses + optional bosses, trigger world phase change, final boss at the bottom.

---

## 2. Mockup Analysis (Source of Truth for Look & HUD)

Three mockup screenshots define the target visual style and UI. **Mockup A** = glowing cave. **Mockup B** = sunlit surface w/ ruined citadel. **Mockup C** = Grid Mode building.

### 2.1 What the mockups establish (mockup-derived requirements)

| ID | Observation | Requirement |
|---|---|---|
| MOC-001 | All three are **first-person** with a visible **viewmodel** (held pickaxe/sword/trowel) and a visible off-hand/arm in B and C. | First-person is default camera; viewmodel with hand(s), sway, swing animations (§12). Off-hand slot supported (§12.6). |
| MOC-002 | Terrain is **faceted low-poly** with large flat-shaded triangles (rock arches, cliffs, cave walls) **but** surface strips (grass edges, dirt) show **pixel-art block-like texture detail**. | Smooth-ish SDF terrain meshed with **flat shading** + **pixel-art triplanar textures** at fixed texel density (§5.5, §9.2). |
| MOC-003 | Terrain edges at cliffs/grass show **cubic stepped voxel look** in places. | Surface-nets cell size ≈ block size so faceting echoes cubes; look-dev spike in M1 decides final cell size (§23 D-01). |
| MOC-004 | Mockup A is **very dark** with strong *colored* local light: orange torches (embers/sparks), cyan/purple bioluminescent mushrooms and crystals. | Voxel-grid colored light propagation + dynamic point lights + emissive materials + bloom (§9.3–9.5). |
| MOC-005 | Mockup B has **volumetric god rays**, floating leaf particles, atmospheric depth haze, distant rock arches and trees visible hundreds of meters away. | Volumetric fog/light shafts, ambient particles, long view distance with distance fog + LOD (§9.6, §19). |
| MOC-006 | **HUD top-left**: row of **hearts** (9 icons, last one half) and below it a row of **5 blue stars** (mana, assumed). | Health = hearts w/ half-heart granularity; mana = stars (§12.2). |
| MOC-007 | **Hotbar** under the stars: **7 visible slots** (pickaxe, sword, hammer, wood planks ×3, torch ×0, potion ×0, potion ×2), selected slot highlighted with its **number** below, **stack counts** at slot bottom-right. 0-count items still show (dimmed). | Hotbar with number keys; show zero-count favorites as ghosted “slot memory” (§14.2). |
| MOC-008 | **Minimap bottom-right**, square, ornate brown/orange frame. In A it shows a **dark underground cross-section with light-grey passages and a yellow player arrow**; in B/C it shows a **top-down terrain render** with the citadel. | Context-sensitive minimap: surface = top-down color render; underground = depth-slice map (§15.3). |
| MOC-009 | **Mode legend above minimap**: `Terrain mode [key]` and `Grid mode [key]` with keycap icons. | Two editing modes with rebindable keys, displayed persistently (§6). |
| MOC-010 | **Mockup C toast**: `Grid mode: Active` (green) over `Placement [Stone Block]`, bottom-center. | Mode-change toast + current placement block name (§6.3). |
| MOC-011 | **Cyan holographic ghost cube** with the block’s texture, outlined in glowing cyan; **cyan grid lines** drawn on nearby ground/block surfaces. | Placement preview ghost + localized grid overlay (§6.3). |
| MOC-012 | Player-built structure uses **wood-plank walls** and **stone/metal-paneled blocks** with distinct textures on adjacent cells; blocks sit directly on/into sloped grassy terrain without gaps. | Block layer interpenetrates terrain (§5.7). Multiple block materials with varied face textures. |
| MOC-013 | **Crosshair** (small “+”) visible in C at screen center. | Center crosshair, toggleable/customizable (§14.1). |
| MOC-014 | Enemies: **swarm of black spiders with glowing eyes**; **hulking stone/rune brute with glowing cyan runes**; **skeleton w/ round shield**; **red winged imps**; **green goblin-like brute**; **gargoyle statues** atop a gatehouse. | Enemy roster seeds (§13.3). Statues may animate (ambush enemy). |
| MOC-015 | Environmental props: **glowing mushrooms** (cyan/purple), **crystal clusters**, **stalactites**, **wooden scaffolding + brick structures + crates** in a cave, **chained arched door** and **purple hanging lanterns** on citadel. | Prop/structure kits (§8.4); “abandoned outpost” cave POI; “Citadel” surface dungeon. |
| MOC-016 | The sword in B has **glowing runes** on the blade. | Runes/enchant cosmetic + gameplay system (§12.8). |
| MOC-017 | Items seen: **pickaxe, sword, hammer, wood planks, torch, red potion(s)**, stone block, a trowel-like grid tool. | Initial item set (§12.4). |
| MOC-018 | Held light/embers: sparks fly from lit torches/fights in A. | Particle FX on lights and combat (§9.7). |

### 2.2 Visual target summary (art direction contract)
- **Geometry**: faceted low-poly (flat normals), chunky silhouettes, large readable shapes. Trees have **cubic leaf clusters** on trunks; grass is **blocky blades/tufts** (instanced props).
- **Textures**: pixel art, **16×16 px per 1 m face** (16 px/m *(tunable)*), nearest-neighbor magnification. No smooth PBR noise textures. One shared texel density across terrain, blocks, props, creatures.
- **Lighting**: HDR, bloom, filmic tonemap, colored lights, strong contrast between lit and dark. Cascaded sun shadows on surface; volumetric shafts at golden-hour angles.
- **Palette**: saturated fantasy. Surface = warm greens/golds; Glowdeep = teal/violet on near-black; ruins = desaturated brick with magenta lantern accents.
- **UI**: pixel-art icons, hand-drawn frame for minimap (ornate bronze), crisp pixel font with smooth fallback for localization.

### 2.3 Ambiguities & interpretations (owner: please confirm; defaults chosen)
| # | Ambiguity | Default interpretation implemented | Alternative |
|---|---|---|---|
| A-1 | Do `Terrain mode` / `Grid mode` act as **mutually exclusive modes** or **independent toggles**? | **Mutually exclusive `EditMode` enum** `{Terrain, Grid}`; Terrain is default; pressing a mode’s key toggles into it, pressing again returns to Terrain. | Two independent booleans. |
| A-2 | What do the **5 blue stars** represent? | **Mana** (magic resource). | Stamina / armor. |
| A-3 | Hearts: 9 icons shown (8.5 filled). | Mockup shows a mid-game state. **Start with 5 hearts**, up to **20** via collectible Life Crystals. HP unit = half-heart = 10 HP *(tunable)*. | Start at 9 or 10 hearts. |
| A-4 | Two red potions: one at ×0, one at ×2. | Slot 6 = **Healing Potion (red)**, slot 7 = **Healing Potion**, i.e. mockup uses same icon. Real game: **Healing = red**, **Mana = blue** (distinct colors). | Both are different red variants. |
| A-5 | Held item in C looks like a **trowel** while hotbar highlights **wood planks** and toast says `Stone Block`. | In Grid Mode the viewmodel swaps to a **Builder’s Trowel**; placement block comes from the **selected hotbar block item**. Mockup mismatch is treated as a work-in-progress composite. | Dedicated placement palette. |
| A-6 | Green line under selected slot in C. | Treated as **selection highlight**; item durability/cooldown bars are separate slot overlays. | It is a durability bar. |
| A-7 | Hotbar shows 7 slots only. | **10 hotbar slots** (keys `1`–`0`); mockup is cropped/early. UI scales to 7–10 depending on width. | Fixed at 7. |
| A-8 | Is the world limited to the surface + caves, or does it include sky? | Include **sky islands** (P2). | Skip. |

---

## 3. Technology Decision: Rust + Bevy vs. C++

### 3.1 Recommendation
**Use Rust with Bevy.** Keep all game logic in Bevy-independent crates so Bevy’s API churn stays contained. Choose C++ only if one of the “flip conditions” in §3.4 is true.

### 3.2 Comparison for *this* game

| Criterion | Rust + Bevy | C++ (custom engine, or SDL3/bgfx/Vulkan) | Verdict |
|---|---|---|---|
| **AI-agent feedback loop** | Strong type system, borrow checker, and precise compiler errors let agents self-correct; `cargo` = one command for build/test/bench/doc | Build systems (CMake/vcpkg/Conan), UB, header issues, and ABI/lib mismatches cost many more iterations; memory bugs hide | **Rust** |
| **Memory/thread safety for chunk streaming** | Data races are compile errors; fearless parallel meshing with `rayon`/task pools | Manual discipline; races/UB in async chunk code are the #1 bug class | **Rust** |
| **Time-to-first-pixel** | Windowing, input, ECS, wgpu rendering, assets, audio, gamepad, UI all included | You build all of it (the owner enjoys this, but it is months of work before gameplay) | **Rust** |
| **Fit for many simulated entities** | ECS is a natural fit (mobs, items, projectiles, particles) | Possible, but you design/maintain your own ECS | **Rust** |
| **Custom render passes / shaders** | Possible (WGSL, custom materials, render graph) but API changes often and render internals are complex | Total control; direct Vulkan/DX12 | C++ (control) |
| **Ecosystem stability** | Pre-1.0; **breaking changes every release** (~every 4–5 months; 0.19 landed June 2026) | Very stable libraries | C++ |
| **Editor / tooling** | No official editor yet; code-first workflow; we build small CLI tools | Roll your own, or use an engine that has one | Tie |
| **Compile times** | Slow clean builds; mitigated with dynamic linking, `mold`/`lld`, Cranelift dev backend | Also slow with heavy templates; mitigated with unity builds, ccache | Tie |
| **Cross-platform (Win/Linux/macOS/Deck)** | wgpu → Vulkan/DX12/Metal; Wayland+X11 via winit | Must write/maintain backends | **Rust** |
| **Prior-art for voxel/SDF meshing** | Good crates exist (`fast-surface-nets`, `block-mesh`, `ndshape`) | Excellent prior art in C++ (Transvoxel, OpenVDB, etc.) | Tie |

### 3.3 Implications of choosing Bevy (agents MUST internalize)
- **Pin an exact Bevy version** in `Cargo.toml` (`=0.19.x` as of Oct 2026 — *verify latest patch at project start*) and **do not upgrade mid-milestone**. Upgrades happen only between milestones, in a dedicated PR/branch using the official migration guide.
- **Ecosystem crates lag Bevy releases.** Before adding any Bevy plugin crate, confirm it supports the pinned Bevy version. If not, prefer writing a small in-house module.
- **Keep Bevy at the edges**: `us_game`, `us_render`, `us_ui`, `us_audio` depend on Bevy; everything else does not (§4.2). This makes a future engine swap, headless server, tooling, and fast unit tests easy.
- **Rendering**: build terrain/block meshes as plain buffers in `us_mesh`, convert to Bevy `Mesh` in `us_render`. Use a custom `Material` (WGSL) for terrain/blocks (§9.2).
- **Compile-time hygiene**: enable `bevy/dynamic_linking` for dev only, use a fast linker, put `opt-level = 1` for the workspace and `3` for dependencies in dev profile.

### 3.4 Flip conditions → choose C++
Pick C++ only if the owner confirms **any** of:
1. They want to **reuse RetroNode** (their existing C++ engine) as the foundation and are willing to extend it to 3D (renderer, 3D physics-free SDF collision, chunk streaming, etc.).
2. They want **total control of the render pipeline** (custom GPU-driven terrain, bespoke virtual texturing) more than they want fast gameplay iteration.
3. They require a **console target** with a mature C++ toolchain.

If C++ is chosen, mirror the crate boundaries in §4.2 as static libraries with the **same engine-agnostic core**, use **CMake + vcpkg**, **SDL3** for windowing/input, **Vulkan (or bgfx/wgpu-native)** for rendering, **EnTT** for ECS, **Jolt** only if rigid bodies are later required, **fmt/spdlog**, **nlohmann/json or RapidJSON** (RON replaced by JSON/TOML), **Catch2/doctest**, and **Tracy** for profiling. Everything else in this spec still applies.

### 3.5 Candidate dependencies (Rust) — *verify maintenance + Bevy compatibility before adopting*
| Purpose | Candidate crate(s) | Notes |
|---|---|---|
| SDF → mesh | `fast-surface-nets`, `ndshape` | Padded chunk arrays; flat-shade after meshing |
| Cube mesh | `block-mesh` or in-house greedy/culled mesher | In-house is fine; cube meshing is small |
| Noise | **in-house** deterministic noise in `us_core` | Cross-platform determinism (§4.4) |
| Math | `glam` (Bevy’s) | Keep pure-math in `us_core` using `glam` |
| Parallelism | `rayon` (pure crates), Bevy `AsyncComputeTaskPool` (game) | |
| Serialization | `serde`, `ron`, `bincode`/`postcard`, `lz4_flex` or `zstd` | RON for data files; binary + compression for saves/network |
| Particles | `bevy_hanabi` (GPU) | Fallback: CPU instanced quads in-house |
| UI | Bevy UI (`bevy_ui`) for HUD; optionally `bevy_egui` **for dev tools only** | Shipping HUD must be Bevy UI/custom |
| Input | Bevy input + `leafwing-input-manager` or `bevy_enhanced_input` | Needs rebinding + gamepad |
| Audio | Bevy audio or `bevy_kira_audio` / `bevy_seedling` | Needs buses, 3D positional, filters |
| Networking | `bevy_replicon` (+ renet backend), `lightyear`, or custom on `quinn`/`renet` | Decide in M0 spike (§23 D-04) |
| Mesh simplify | `meshopt` | For optional far-LOD / facet-merge look |
| Localization | `fluent` / `fluent-bundle` | |
| Testing | `proptest`, `insta`, `criterion` | |
| CLI | `clap` | |
| Logging | `tracing`, `tracing-subscriber` | JSON logs for agents |
| Steam | `steamworks` | P2 |
| Physics (optional) | `avian3d` / `bevy_rapier3d` | **Not** used for terrain collision (§7.1) |

---

## 4. Architecture

### 4.1 High-level model: authoritative simulation + presentation shell
```
┌──────────────── CLIENT ────────────────┐        ┌──────────── SERVER (authoritative) ───────────┐
│ Input → Intent msgs ─────────────────────net───▶│ Validates intents → mutates World state       │
│ Prediction (player move, dig preview)  │        │  • Terrain/Block edit ops (§5.6)              │
│ Chunk mesh builder (async)             │◀───────│  • Entities (mobs, items, projectiles)        │
│ Render, UI, Audio, Particles           │ deltas │  • Fluids, light, AI, spawning, worldgen      │
└────────────────────────────────────────┘        └───────────────────────────────────────────────┘
Single-player = same binary running a LOCAL SERVER in-process, connected over an in-memory channel.
```
**MUST (P0, M0–M3)**: even in single-player, gameplay mutations flow as **intent → server system → authoritative state → replicated delta**. This is the only way multiplayer stays tractable.

### 4.2 Workspace / crate layout
```
understrata/
├─ Cargo.toml                 # [workspace], pinned versions, profiles
├─ rust-toolchain.toml
├─ crates/
│  ├─ us_core/       # NO Bevy. IDs, registries, deterministic RNG+noise, math helpers, coords, fixed tick types
│  ├─ us_content/    # NO Bevy. serde schemas for all data files, loaders, validators, registries (ItemDef, BlockDef, …)
│  ├─ us_voxel/      # NO Bevy. Chunk storage (SDF + material), block layer, edit ops, raycasts, palette compression
│  ├─ us_mesh/       # NO Bevy. Surface-nets/terrain mesher, block mesher, flat-shading, AO bake, LOD → plain buffers
│  ├─ us_worldgen/   # NO Bevy. gen_chunk(seed, coord), biomes, caves, ores, structures, vegetation
│  ├─ us_sim/        # NO Bevy. Light propagation, fluid CA, nav graph, damage/stat rules, loot rolling, spawn director
│  ├─ us_net/        # NO Bevy. Protocol messages, op serialization, versioning
│  ├─ us_game/       # Bevy. Gameplay plugins/systems (player, combat, inventory, AI glue, server/client schedules)
│  ├─ us_render/     # Bevy. Terrain/block materials, shaders, post-FX, particles, viewmodel pass
│  ├─ us_ui/         # Bevy. HUD, menus, inventory/crafting UI, map, settings
│  ├─ us_audio/      # Bevy. Audio buses, adaptive music, positional SFX, reverb/occlusion
│  ├─ us_tools/      # CLI: validate-content, bake-atlas, dump-chunk, worldgen-preview, bench, replay
│  └─ understrata/   # Bin: client (default), `--server` dedicated, `--headless` test mode
├─ assets/
│  ├─ data/          # RON: items/, blocks/, recipes/, enemies/, loot/, biomes/, structures/, balance/, keybinds/
│  ├─ textures/ models/ audio/ shaders/ lang/ fonts/
├─ docs/             # ARCHITECTURE.md, DECISIONS.md, OPEN_QUESTIONS.md, DEPENDENCIES.md, ART_BIBLE.md
├─ tests/ benches/ scripts/
└─ CHANGELOG.md
```
Rules: dependency arrows only point **downward** (`us_game` → `us_sim` → `us_voxel` → `us_core`). No cycles. `us_core`…`us_net` MUST compile with **no Bevy and no GPU**.

### 4.3 Tick model (fixed timestep) *(tunable)*
| System group | Rate | Notes |
|---|---|---|
| Player movement / collision / projectiles | **60 Hz** fixed | Render interpolates |
| Combat resolution, damage | 60 Hz | Deterministic order |
| Mob AI decision | 10–20 Hz (staggered) | Steering at 60 Hz |
| Fluid CA | 10 Hz (active chunks only) | Budgeted ms/tick |
| Light propagation | event-driven (on edit) | Budgeted ms/frame |
| Nav graph rebuild | event-driven, async | |
| Spawn director | 1 Hz | |
| Autosave | every 120 s *(tunable)* | Async, atomic |
| Day/night, weather | 1–4 Hz | |

### 4.4 Determinism rules (worldgen & edit replay)
- `gen_chunk(seed, chunk_coord) -> ChunkData` MUST be a **pure function**: no global state, no wall-clock, no thread-order dependence.
- Use **integer hashing** (e.g., SplitMix/PCG variants) for randomness, **in-house gradient noise**, and the **`libm` crate** (not `std`) for `sin/cos/exp/powf/ln` in worldgen so results do not vary by platform libm.
- Cross-chunk features (caves, rivers, structures, ore veins) MUST be generated by **region-hashed placement**: pick feature origins from a hash of `(seed, region_coord, feature_kind)` on a coarse grid (e.g., 128–256 m regions) and let each chunk evaluate every feature whose bounding box overlaps it. This guarantees chunks generate **independently and consistently**.
- Golden tests: `hash(chunk bytes)` for fixed seeds/coords stored in `insta` snapshots (§22.2). Any intentional worldgen change bumps `WORLDGEN_VERSION` and regenerates snapshots in the same PR.
- Edit ops are deterministic given identical prior state (use integer/quantized parameters, not floats, in the *stored* op log — §5.6).

### 4.5 Coordinate systems & constants
```rust
pub const CELL: f32 = 1.0;                 // meters; terrain sample spacing == block size (tunable via D-01)
pub const CHUNK_DIM: i32 = 32;             // cells per chunk edge
pub const CHUNK_PAD: i32 = 1;              // 1-sample border for seamless meshing (34³ padded array)
pub const WORLD_MIN_Y: i32 = -384;         // bedrock floor
pub const WORLD_MAX_Y: i32 =  128;         // sky ceiling
pub const SEA_LEVEL_Y: i32 = 0;
pub const PLAYER_HEIGHT: f32 = 1.8; pub const PLAYER_RADIUS: f32 = 0.35; pub const EYE_HEIGHT: f32 = 1.62;
```
Types: `WorldPos(Vec3)`, `CellPos(IVec3)`, `ChunkPos(IVec3)`, conversions via `div_euclid/rem_euclid` (never `/` `%` on negatives).

### 4.6 World size presets
| Preset | X × Z (m) | Vertical (m) | Chunks (approx) | Priority |
|---|---|---|---|---|
| Small | 1024 × 1024 | 512 | 32×32×16 = 16,384 | P0/P1 |
| Medium | 2048 × 2048 | 512 | 65,536 | P1 |
| Large | 4096 × 4096 | 512 | 262,144 | P2 |
| Custom | user-defined (≤ Large) | | | P2 |

World edges: soft barrier (impassable cliffs/ocean + invisible wall at the boundary), no wrap-around.

---

## 5. Terrain System (SDF Terrain)

### 5.1 Representation
Terrain is a **signed distance field (SDF)** sampled on the integer lattice (spacing `CELL`). **Negative = solid, positive = air** (matches `fast-surface-nets`). Stored per chunk with a parallel **material** field.

```rust
pub enum Field<T> { Uniform(T), Dense(Box<[T]>) }          // collapse uniform chunks (all-air / all-stone) to one value

pub struct TerrainChunk {
    pub pos: ChunkPos,
    pub sdf: Field<i8>,        // CHUNK_DIM^3 owned samples; quantized: d = value as f32 * SDF_QUANT
    pub mat: Field<u8>,        // MaterialId per sample (dominant material around that sample)
    pub version: u32,          // ++ on every edit; used by net sync + mesh cache invalidation
    pub flags: ChunkFlags,     // GENERATED | EDITED | MESH_DIRTY | LIGHT_DIRTY | NAV_DIRTY | SAVE_DIRTY
}
pub const SDF_QUANT: f32 = CELL / 32.0;   // i8 range ≈ ±3.97 cells; clamp on write
```
- **MUST** gather a **padded 34³ immutable snapshot** (self + neighbor border samples) for each mesh job so edits can continue on the main thread while a job runs.
- Chunks with `Uniform` fields **MUST NOT** allocate dense arrays (most of the world is uniform air or solid).
- Sampling API (in `us_voxel::query`, used by physics/AI/rays): `sdf_at(Vec3) -> f32` (trilinear), `gradient_at(Vec3) -> Vec3`, `material_at(Vec3) -> MaterialId`, `is_solid(Vec3) -> bool`.

### 5.2 Meshing (low-poly look)
- **Algorithm**: naive **Surface Nets** per chunk (crate `fast-surface-nets` or in-house). Output **unindexed triangles with flat (face) normals** for the faceted look.
- Per triangle: material = majority of the 8 corner materials of its source cell (ties → higher `priority`); **per-vertex AO** (baked from SDF neighborhood, §9.4); **light** sampled from the light grid (§9.3) at vertex.
- Skirts / seams: chunk borders MUST be crack-free at same LOD (guaranteed by the 34³ padded snapshot). Cross-LOD stitching (P1, M15): **skirts** (vertical flaps) are acceptable; Transvoxel is *not* required for 1.0.
- **Low-poly look-dev spike (TER-003, P0, M1)** — implement all three and screenshot-compare against the mockups, then record the decision in §23 D-01:
  - **(A)** 1 m cell, flat shading (baseline).
  - **(B)** 1 m edit lattice, **render mesh simplified** (coplanar merge/`meshopt`, error ≈ 0.15–0.3 m) for larger facets like the mockup rocks.
  - **(C)** 1 m lattice with **vertex position quantization** to 0.25 m grid for chunkier silhouettes.
- Surface Nets rounds sharp corners. This is accepted for P0/P1. **Crisp cubic cuts** are researched in TER-016 (P2).

### 5.3 Terrain materials (data: `assets/data/materials/*.ron`)
| Field | Description |
|---|---|
| `id`, `name_key` | stable string id (e.g., `us:stone`), loc key |
| `hardness` | dig difficulty (float) |
| `min_tool_tier` | minimum pickaxe tier to dig at all (int) |
| `yield` | `{ item, per_m3 }` |
| `textures` | `{ top, side, bottom }` texture-array layer ids |
| `sound_set` | footstep/dig/place/impact sounds |
| `priority` | tie-break for mixed cells |
| `emission` | optional glow (RGB, strength) for glowing ores/crystals |
| `tags` | `["ore","stone","soil","fluid_blocker",…]` |
| `slippery`, `damage_on_touch`, `flammable` | gameplay flags |

**Starter material set (working names)**: `dirt, clay, sand, sandstone, gravel, snow, ice, mud, stone, dense_stone, ruin_stone, basalt, obsidian, voidrock, bedrock (indestructible), copper_ore, iron_ore, silverite_ore, aurum_ore, magmite_ore, voidsteel_ore, glow_crystal, gem_ore_*`.

### 5.4 Surface skins (procedural overlays)
Computed in the mesher (no simulation): up-facing (`normal.y > 0.7`) sky-exposed `dirt` → **grass** top; cold biome → **snow**; damp/dark cave dirt → **moss**; sandy → **dry crust**. A narrow **fringe band** on side faces MAY be blended via per-vertex `skin_factor` to reproduce the grass-lip look in Mockup B. Skins change automatically when terrain is edited (digging a grass hill reveals dirt; covering it hides grass).

### 5.5 Texturing (Mockup-derived MOC-002)
- Texture arrays per face role (top/side/bottom). **Triplanar** projection using **world-space** UVs at `TEXELS_PER_M = 16` *(tunable)* → textures never “swim” when terrain is edited.
- Sampler: **mag = nearest**, **min = linear + mipmaps**, anisotropy 8, small negative mip bias to keep crispness.
- Triplanar blend uses a sharp exponent (≥ 8) to avoid mushy blends; pick the dominant axis hard-edged when `blend_sharpness` high (pixel-art look).
- Per-vertex material weights (up to 2 materials/vertex) allow a 2-px dithered transition between materials (ordered dither in shader) — avoids blurry blends.
- Emissive materials (glow crystals, magma) write to an emission channel feeding bloom.

### 5.6 Edit operations (authoritative, deterministic, loggable)
```rust
pub enum EditOp {
    TerrainBrush { center_q: IVec3 /*1/16 m*/, radius_q: u16 /*1/16 m*/, shape: BrushShape, mode: BrushMode,
                   material: MaterialId, strength: u8 },
    TerrainBox   { min: IVec3, max: IVec3 /*cell coords*/, mode: BoxMode, material: MaterialId },   // grid-mode cuts/fills
    BlockSet     { cell: IVec3, state: BlockStateId },                                                // 0 = air
    BlockFill    { min: IVec3, max: IVec3, state: BlockStateId, replace: bool },
    FluidSet     { cell: IVec3, kind: FluidKind, level: u8 },
}
pub enum BrushMode { Subtract, Add, Smooth, Flatten { plane_q: IVec4 }, Paint }
pub enum BrushShape { Sphere, Cylinder { axis: Axis, half_h_q: u16 }, Cube }
pub struct LoggedOp { pub seq: u64, pub tick: u64, pub actor: PlayerId, pub op: EditOp }
pub fn apply(world: &mut World, op: &EditOp) -> Applied { /* dirty chunks, volume delta per material, spawned drops */ }
```
- **Fixed-point ints** in ops so replays are bit-exact. `apply` is a **pure function of (state, op)**.
- Subtract: `d' = max(d, -brush_sdf)`; Add: `d' = min(d, brush_sdf)`; with optional **smooth blend** (`k ≈ 0.25·CELL`) for rounder results. Quantize + clamp to `i8` after.
- **Dirty propagation**: mark the edited chunk; also mark neighbors only if the edit touches border samples (±1).
- **Remesh scheduler**: priority = (player-initiated) > (distance to camera). At most **1 in-flight job per chunk**; if re-edited mid-job, re-queue on completion. **Keep the old mesh until the new one is ready** (no flicker/holes).
- **Undo/redo**: store inverse patches (changed sample region, RLE). Creative: 64 steps *(tunable)*; Survival: “Undo last placement” for the last 20 block placements within 30 s with refund (P1).

### 5.7 Terrain ↔ Block interplay (the “Overlay Model”) — **normative**
1. Terrain and block layers are **independent and may interpenetrate**. Blocks never write to the SDF. Terrain edits never delete blocks.
2. **Placement validity** of a block in cell `c`: no block already in `c`; `c` is **not fully solid** (at least 1 of its 8 corner samples is air); no overlapping solid entity (player/mob; items excluded); within reach; (optional strict mode P2: needs adjacent support).
3. Block mesher culls faces only against **neighbor blocks**. Faces next to terrain are kept (hidden naturally where terrain is solid). To prevent z-fighting on coplanar contact, **inflate block meshes by `BLOCK_EPSILON = 0.002 m`** and apply a small depth bias to terrain.
4. **Collision** = union of terrain + blocks (§7.1). **Light** treats blocks as opaque/transparent per `BlockDef` and combines with terrain solidity (§9.3).
5. Digging terrain beneath blocks leaves blocks in place (floating allowed in 1.0; optional “support rule” P3).
6. Optional optimization (P2): skip emitting terrain triangles that lie fully inside opaque block cells.

### 5.8 Dig mechanics (data: `assets/data/balance/mining.ron`)
- **Bite model** (juicy, discrete): while primary held, every `bite_interval` (default 0.25 s ÷ speed multiplier) apply a `TerrainBrush{Subtract}` sphere.
- **Brush center** = raycast hit point pushed **inward** along `-normal` by `0.4 × radius`. Default pickaxe radius **1.25 m**; upgrades up to 2.5 m. **Reach** 4.5 m *(tunable)*.
- **Speed** = `clamp(tool.power / material.hardness, 0.25, 4.0)`. If `tool.tier < material.min_tool_tier` → **no dig**, play “clink” + sparks + toast `Too hard — need a better pickaxe`.
- **Yield accumulator** per (player, material): `acc += removed_m3 * material.yield.per_m3`; when `acc ≥ 1` spawn that many item drops with small velocity toward the player (**magnet**). Prevents item spam from tiny edits and supports any brush size.
- **Fill**: consumes 1 item per `fill_m3_per_item` (default 4 m³) *(tunable)*; Creative = free.
- **Feedback**: bite particles (debris cubes colored by material), dust puff, camera micro-shake (settings-scalable), viewmodel swing, per-material SFX and haptics.
- **Bedrock** and `indestructible` materials: never dig.

### 5.9 Terrain feature list
| ID | Feature | Pri | Milestone | Acceptance criteria |
|---|---|---|---|---|
| TER-001 | SDF chunk storage w/ uniform collapse | P0 | M1 | Memory for uniform chunk < 64 B; unit test for round-trip; 1k random edits don’t grow uniform chunks unnecessarily |
| TER-002 | Surface-nets mesher w/ flat shading + materials | P0 | M1 | Watertight mesh property test (no holes, no cracks at chunk borders) over random SDFs |
| TER-003 | Look-dev spike A/B/C | P0 | M1 | Screenshot set + decision ADR |
| TER-004 | Dig bite brush w/ reach, hardness, tool tiers | P0 | M2 | Digging `dense_stone` (min tier 2) with a wood pickaxe (tier 1) fails with feedback; copper pickaxe (tier 2) succeeds; speed scales with power/hardness |
| TER-005 | Fill brush w/ item cost | P0 | M2 | Place dirt blob; consumes items; Creative free |
| TER-006 | Smooth / Flatten / Paint brushes (Terraform Wand family) | P1 | M4 | Flatten produces level pad of radius r at plane height ±0.1 m |
| TER-007 | Async remesh scheduler w/ priority + no-flicker swap | P0 | M2 | Edit→visible < 100 ms p95; zero frames with missing chunk meshes in soak test |
| TER-008 | Yield accumulator + drops w/ magnet | P0 | M2 | Total items dropped within ±1 of expected for scripted dig |
| TER-009 | Edit-op log (seq-ordered, replayable) | P0 | M2 | Replaying log on fresh world yields identical chunk hashes |
| TER-010 | Ore veins w/ sparkle shader + detection items | P1 | M11 | Ores visible via glint at ≤ 12 m in dark; “Spelunker” effect highlights ≤ 40 m |
| TER-011 | Surface skins (grass/snow/moss/crust) | P0 grass / P1 rest | M1 / M11 | Grass appears on sky-exposed up-facing dirt and disappears when covered |
| TER-012 | Bedrock/indestructible handling | P0 | M2 | Bedrock cannot be modified by any op incl. explosions |
| TER-013 | Terrain lock (area protection) | P2 | M13 | Protected region rejects ops from non-owners (MP) |
| TER-014 | Floating-fragment cleanup | P2 | M14 | Fragments < N cells removed/dropped after edit; no hitch > 5 ms |
| TER-015 | Far-LOD w/ skirts | P1 | M15 | View distance 16 chunks at min-spec Low preset hitting 60 FPS |
| TER-016 | Crisp-edge box cut (research) | P2 | M14 | Spike report: DC vs. triangle clipping; decide |
| TER-017 | Tunneling creatures edit terrain (server-side ops) | P2 | M14 | Burrower enemy leaves tunnels; ops replicate in MP |
| TER-018 | Explosives & meteor craters (sphere subtract + knockback) | P1 | M12 | Bomb removes sphere r=3m, respects bedrock, drops items per yield |
| TER-019 | Dig/placement juice (particles, SFX, shake, haptics) | P0 | M2 | Per-material variants exist for stone, dirt, ore, sand |
| TER-020 | Terrain undo/redo (Creative) | P1 | M4 | Undo restores exact chunk hashes for ≥ 64 steps |

---

## 6. Edit Modes & Building System (Grid Mode)

### 6.1 Mode model (resolves ambiguity A-1)
```rust
pub enum EditMode { Terrain, Grid }   // default Terrain
```
- Rebindable keys; defaults **`V` = Terrain**, **`G` = Grid**. Pressing the key of the *current* non-default mode returns to Terrain.
- HUD legend (MOC-009) shows both keys at all times; toast on change (MOC-010): `Grid mode: Active` in green / `Terrain mode: Active` in gold.
- Mode is **per-player client state**; ops sent to the server carry the already-resolved op (server does not care about mode).

### 6.2 Behavior matrix
| Input / Context | **Terrain Mode** (organic, fast) | **Grid Mode** (precise, constructive) |
|---|---|---|
| Overlay | none | **Cyan grid lines** around cursor (radius 6 m, fading) + ghost preview |
| Primary w/ **pickaxe** | Dig sphere bites in terrain; ore/rock mining | On a **block**: mine that block (whole cell). On terrain: **box-cut one cell** (cell-aligned excavation; P1, rounded by surface nets until TER-016) |
| Primary w/ **hammer** | Smash breakables; quick-remove nearby blocks | Instant-remove block; hold `Shift` for **shape-cycle** (full→slab→stair→slope…) |
| Secondary w/ **block item** | Place block in adjacent free cell (no overlay; quick) | Place at **snapped cell** with ghost; **drag tools**, plane-lock, rotate, replace |
| Secondary w/ **terrain-fill item** (dirt/sand/stone) | Add smooth terrain blob (organic) | Add **cell-aligned fill** (P1) |
| Scroll / number keys | change hotbar slot | same; **`Alt`+scroll** cycles block variants (shape/orientation) |
| Viewmodel | selected item | **Builder’s Trowel** (A-5) + selected block floats in off-hand |
| Targeting | camera ray vs terrain SDF/blocks | **DDA cell ray** (face-aware) |

### 6.3 Grid Mode requirements
| ID | Feature | Pri | M | Acceptance criteria |
|---|---|---|---|---|
| BLD-001 | `EditMode` state, toggle, rebindable keys, HUD legend + toast | P0 | M4 | Mode survives hotbar swaps; legend always visible; toast on change |
| BLD-002 | **Grid overlay shader** (uniform-driven: cursor pos, radius, color) on terrain + blocks, no extra geometry | P0 | M4 | Lines align to integer meter boundaries on any surface; fades by distance |
| BLD-003 | **Cell targeting** via DDA; returns `(hit_cell, face_normal, place_cell)` | P0 | M4 | Unit tests incl. negative coords, grazing rays, max reach |
| BLD-004 | **Ghost block** with real textures, cyan outline; **red** when invalid | P0 | M4 | Updates every frame; invalid reasons listed in debug overlay |
| BLD-005 | Place/remove block ops through server (EditOp::BlockSet) | P0 | M4 | Round-trip in MP and SP; items consumed/refunded |
| BLD-006 | Block chunk storage + mesher (culled faces, per-face textures, AO, light) | P0 | M4 | 10k blocks placed → mesh p95 < 3 ms/chunk |
| BLD-007 | **Rotate** (`R`), **flip** (`F`), **shape cycle** (`C`) | P1 | M5 | Orientation persisted in `BlockState` |
| BLD-008 | **Drag-place**: line (default), rectangle (`Ctrl`), box (`Ctrl+Shift`), preview count, `Esc` cancel | P1 | M5 | Cost preview matches consumed items |
| BLD-009 | **Plane lock** (`Alt` hold) to draw flat floors/walls | P1 | M5 | Placement constrained to locked plane |
| BLD-010 | **Replace mode** (place over existing block; refund old) | P1 | M5 | Atomic single op |
| BLD-011 | **Eyedropper** (`MMB`) picks block from inventory (Creative: from palette) | P1 | M5 | Selects hotbar slot containing item |
| BLD-012 | Block **shapes**: full, slab (top/bottom), stair, corner stair, slope, pillar, arch, fence, wall, door, trapdoor, window, ladder | P1 | M5–M11 | Each has collision AABBs, light rules, mesh, sound |
| BLD-013 | **Furniture/stations** as `BlockEntity` (workbench, furnace, anvil, chest, bed, table, chair, lantern, sign, waystone) | P1 | M5–M11 | Interaction prompt `[E]`; state persisted |
| BLD-014 | **Hammer**: fast demolish + shape cycle | P1 | M5 | Hammer removes block 2× faster than pickaxe |
| BLD-015 | **Symmetry** (mirror 1–3 axes; radial N-fold) | P2 | M14 | Mirror ops generated client-side; server validates each |
| BLD-016 | **Blueprints**: select region → save → paste ghost; share as file | P2 | M14 | Paste costs materials in Survival; free in Creative |
| BLD-017 | **Block variants** via dye/stain (color tint as palette index) | P2 | M14 | Persisted in `BlockState` |
| BLD-018 | Wall decor props (banners, paintings, trophies) attached to block faces | P2 | M14 | Removing support block drops decor |
| BLD-019 | Doors/levers/pressure plates (+ simple logic blocks) | P2 / P3 logic | M12 / post | Door open/close replicated; logic wires = P3 |
| BLD-020 | Strict build mode (requires support) as optional world rule | P3 | post | |
| BLD-021 | Creative-mode **WorldEdit-style** tools: select, copy, paste, fill, replace, undo | P1 | M14 | Works through `EditOp::BlockFill` batches |

### 6.4 Block definition (data: `assets/data/blocks/*.ron`)
```ron
BlockDef(
  id: "us:stone_brick",
  name_key: "block.stone_brick",
  hardness: 2.0, min_tool_tier: 1,
  opaque: true, emits_light: None, light_filter: 15,       // light_filter: how much light it absorbs (0..15)
  textures: (top: "stone_brick_top", side: "stone_brick_side", bottom: "stone_brick_bottom"),
  shapes: [Full, Slab, Stair, Slope, Pillar],              // allowed shape variants
  sound_set: "stone",
  drops: [( item: "us:stone_brick", count: 1 )],
  tags: ["stone", "building"],
  block_entity: None,
  flammable: false,
)
```

---

## 7. Collision & Movement Physics

### 7.1 Collision strategy — **do NOT use triangle-mesh colliders for terrain**
Reasons: rebuild latency after edits, ghost collisions at chunk seams, cost, and they desync from the SDF. Instead:
- **Capsule vs combined field.** `solid_distance(p) = min(terrain_sdf(p), block_distance(p))` (blocks via per-cell AABB lists; partial shapes = multiple AABBs).
- **Algorithm (kinematic controller, per 60 Hz tick)**:
  1. Integrate desired velocity.
  2. **Sphere-sweep** the capsule’s lower/upper spheres along the motion using sphere tracing on the SDF (bounded step), plus swept AABB tests vs. block AABBs.
  3. On hit, **slide** along the surface (project velocity on tangent plane via SDF gradient); iterate ≤ 4 times.
  4. **Depenetrate** via gradient push-out (≤ 4 iterations) when overlapping after edits.
  5. **Ground probe**: downward capsule cast; compute slope from gradient; **step-up** ≤ 0.6 m; **ground snap** ≤ 0.35 m when descending slopes.
- **Slope rule**: walkable if `normal.y ≥ cos(65°)`; steeper (very steep hills) → slide down.
- **Same controller** for mobs (different capsule radii/heights); flyers use a sphere with 3D steering.
- **Queries** (`us_voxel::query`): `sdf_at`, `gradient_at`, `capsule_overlap`, `sphere_sweep`, `raycast_terrain` (sphere tracing), `raycast_blocks` (DDA), `raycast_all` (merge).

### 7.2 Player movement (data: `assets/data/balance/movement.ron`; all *(tunable)*)
| Param | Default |
|---|---|
| Walk / Sprint / Crouch speed | 4.3 / 6.5 / 2.0 m/s |
| Gravity | 24 m/s² |
| Jump height | ~1.15 m (clears 1-block rise with step-up not needed) |
| Air control | 30 % |
| Coyote time / jump buffer | 100 ms / 100 ms |
| Fall damage threshold | 6 m (halved by water landing; negated by feather/accessory) |
| Swim speed / sink / surface bob | 3.0 m/s / slow / yes |
| Ladder/rope climb speed | 3.0 m/s |
| Step-up height | 0.6 m |

### 7.3 Movement feature list
| ID | Feature | Pri | M | Acceptance criteria |
|---|---|---|---|---|
| MOV-001 | Capsule controller vs SDF+blocks (walk/jump/slope/step/slide) | P0 | M3 | 10-min fuzz run over random terrain: never stuck > 1 s, never tunnels through terrain |
| MOV-002 | Post-edit depenetration (digging under feet / placing block on player) | P0 | M3 | Player ejected to nearest free pose, no teleport > 1.5 m |
| MOV-003 | Sprint, crouch (can’t un-crouch under low ceiling), coyote/buffer | P0 | M3 | |
| MOV-004 | Swim, wade, drown (breath bar) | P1 | M11 | Breath depletes underwater; refill at surface |
| MOV-005 | Ladders, ropes, climbable blocks | P1 | M5 | |
| MOV-006 | **Grappling hook** (“Grapnel”) | P1 | M12 | Latches to terrain/blocks within 20 m, swing + reel |
| MOV-007 | Fall damage + landing feedback | P1 | M8 | |
| MOV-008 | Accessory movement perks (double-jump charm, dash boots, wall-cling gloves) | P2 | M12 | Data-driven modifiers |
| MOV-009 | Glider / parachute | P2 | M14 | |
| MOV-010 | Mounts (P3), boats (P2) | P3 | post | |
| MOV-011 | Free-fly/noclip for Creative and debug | P0 | M1 | `F` toggles in creative; speed modifiers |

### 7.4 Projectiles & hitscan
Swept sphere/ray per tick vs SDF, blocks, and entity hurtboxes. Arrows can **stick** into terrain (decal/entity that persists for N s). Gravity and drag per projectile data. Magic bolts may pierce/bounce/explode (terrain brush on explosion only if flagged).

### 7.5 Limited physics shipped in 1.0
Item drops (bounce, settle, magnet), debris particles, explosions (sphere subtract + impulse), falling sand/gravel *blocks* (P2), cosmetic death ragdoll (P3). **No general rigid-body engine** required for 1.0.

---

## 8. World: Strata, Biomes, Generation, Structures

### 8.1 Strata (vertical layers) — core progression axis *(working names; Y ranges tunable)*
| Stratum | Working name | Y range (m) | Look / mood | Hazards | Enemies (seed) | Key resources | Boss |
|---|---|---|---|---|---|---|---|
| S0 | **Canopy** (surface) | −8 … +128 | Sunlit, god rays, arches, forests (Mockup B) | Night spawns, fall | Imps, goblin-brutes, skeleton patrols near ruins | Wood, dirt, stone, copper (outcrops) | **Citadel Warden** (surface Citadel → descends into S3) |
| S1 | **Rootwarren** | −8 … −64 | Brown/green, roots, clay, tight tunnels | Cave spiders, drops, gas pockets (P2) | **Cave Skitter swarms**, flitters, gel | Copper, iron (shallow), clay | **Brood Queen** |
| S2 | **Glowdeep** | −64 … −140 | Teal/violet bioluminescence on near-black (Mockup A) | Spore clouds (slow/poison), vast caverns | Skitters, sporelings, **Runebound Brute** | Iron, silverite, glow crystals | **Runebound Colossus** |
| S3 | **Forgotten Halls** | −140 … −220 | Ancient brick halls, tombs, chains, magenta lanterns | Traps, locked doors | Skeleton shieldbearers/archers, gargoyle sentinels | Silverite, aurum, relics | (Citadel Warden’s lair) |
| S4 | **Magma Reach** | −220 … −300 | Orange/red, lava lakes, basalt | Lava, heat, fire | Cinder elementals, magma crawlers | Aurum, magmite, obsidian | **Cinder Wyrm** |
| S5 | **Undervoid** | −300 … −384 | Near-total dark, void crystals, eerie hush | Darkness stress, void rifts | Void-touched horrors | Voidsteel, rare gems | **Heart of the Strata** (final) |
Strata boundaries are **blended** (±12 m noise-jittered) so there are no hard lines. Each stratum has its own: material palette, ambient light/fog color, music, SFX reverb preset, spawn table, loot table, structure table, ore table.

### 8.2 Surface biomes *(working names)*
| Biome | Terrain character | Flora/props | Notes |
|---|---|---|---|
| **Verdant Reach** (temperate) | Rolling hills, rock arches, ruins | Cubic-leaf trees, grass tufts, flowers | Default start biome (Mockup B) |
| **Stonecrown Peaks** | Tall faceted mountains, natural arches, overhangs | Pines, scree | Heavy 3D noise for overhangs |
| **Frostmarch** | Snow skin, ice caves | Frost pines, ice crystals | Cold hazard (P2) |
| **Sunscar Dunes** | Sand/sandstone, mesas | Cacti, dry bushes | Heat hazard (P2), buried tombs |
| **Mirewood Fen** | Low wet terrain, mud | Mangrove trees, reeds, fog | Slow terrain, poison water (P2) |
| **Saltglass Coast / Ocean** | Beaches, deep water | Kelp, shells | Boats P2, underwater caves P2 |
| **Skyreach** (sky islands) | Floating islands y +64…+120 | Wind-swept grass | P2; reached via grapple/updraft |
| **The Blight** (spreading corruption) | Dark reskinned terrain, spreads slowly | Twisted growths | P2; world rule toggle; containable by terraforming/purifier |
Biome selection: low-frequency **temperature × humidity × “weirdness”** noise maps (2D) + altitude; transitions blended over ~32 m; world spawn guaranteed in Verdant Reach.

### 8.3 Generation pipeline (all stages pure + deterministic, §4.4)
| Stage | Output | Notes |
|---|---|---|
| 1. Macro height + shape | Base density `d = (H(x,z) − y)·k + N3(x,y,z)·a` | Domain-warped ridged noise for mountains; 3D noise term creates overhangs/arches |
| 2. Strata & materials | Material by depth + noise jitter | Soil layer thickness varies by biome |
| 3. Caves | (a) caverns: low-freq 3D noise threshold; (b) tunnels: ridged-noise “worms”; (c) vertical shafts; (d) stratum-specific cavern styles (Glowdeep = tall chambers + stalactites) | **Connectivity guarantee**: region-hashed *descent shafts* ensure a traversable path surface→S5 |
| 4. Ores & gems | Veins as blobs/ribbons by depth band, rarity, and biome | Data-driven tables |
| 5. Liquids | Ocean at sea level, lakes, cave pools, lava lakes in S4 | Written to the fluid layer (§10) |
| 6. Vegetation & props | Instanced trees/grass/flowers/rocks/mushrooms/crystals | **Not entities**; generated procedurally; “removed prop” tracked in edit delta |
| 7. Structures | POIs from templates (§8.4) | Region-hashed placement, terrain-adaptive |
| 8. Spawn & landmarks | Start camp, waystone ruins | |
| 9. Biome hazards | Blight seeds (P2) | |

### 8.4 Structures, props & POI kits
Structure templates live in `assets/data/structures/*.ron`:
```ron
StructureDef(
  id: "us:citadel_gate",
  size: (24, 18, 16),
  palette: { "a": "us:ruin_brick", "b": "us:ruin_brick_mossy", "g": "us:gargoyle_statue", "l": "us:lantern_purple" },
  layers: [ /* y-slices of strings mapping to palette keys, '.' = air, ' ' = leave terrain */ ],
  markers: [ (kind: Chest, pos: (3,1,5), loot: "us:citadel_common"),
             (kind: Spawner, pos: (8,1,8), table: "us:citadel_guards"),
             (kind: BossAnchor, pos: (12,1,8), boss: "us:citadel_warden") ],
  terrain_adapt: ( carve_interior: true, fill_foundation_depth: 6, smooth_radius: 3.0 ),
  allowed_biomes: ["us:verdant_reach","us:stonecrown_peaks"],
  rotations: [0,90,180,270], mirror: true,
  min_spacing_m: 400, rarity: 0.2,
)
```
**POI list (1.0 target)**
| ID | Structure | Stratum/Biome | Pri | Notes |
|---|---|---|---|---|
| WLD-S01 | **Citadel** (gatehouse w/ gargoyle statues, chained arch door, purple lanterns; Mockup B) | Surface → S3 | P0 (gate only) / P1 (full) | Gargoyles may activate as ambush; leads to S3 dungeon + boss |
| WLD-S02 | **Abandoned Outpost** (wood scaffolding, brick, crates; Mockup A) | S1–S2 caves | P0 | Loot + lore notes |
| WLD-S03 | **Mineshafts** (long timbered tunnels, rails P2) | S1–S2 | P1 | |
| WLD-S04 | **Spider Nest** | S1–S2 | P1 | Brood Queen arena |
| WLD-S05 | **Glowcap Grove** (giant luminous mushrooms) | S2 | P0 | |
| WLD-S06 | **Rune Shrine** (Life/Mana crystal pedestals) | S1–S4 | P1 | |
| WLD-S07 | **Waystone Ruins** | All | P1 | Fast-travel unlock |
| WLD-S08 | **Buried Treasure / Hidden Chests** | All | P1 | |
| WLD-S09 | **Boss Arenas** (per boss) | S1–S5 | P1 | Flat, readable, barrier-capable |
| WLD-S10 | **Ruined Towers / Watchposts** | S0 | P1 | |
| WLD-S11 | **Villages / NPC camps** | S0 | P2 | |
| WLD-S12 | **Sky Island shrines** | Skyreach | P2 | |
| WLD-S13 | **Lava Forges** | S4 | P1 | Hellforge-style crafting station |
| WLD-S14 | **Void Sanctum** (final arena) | S5 | P1 | |

### 8.5 Ambient props (instanced)
Trees w/ **cubic canopy clusters** and blocky trunks; grass tufts/blades (instanced quads/cubes with wind sway shader); flowers; mushrooms (multi-size, emissive); crystals (emissive clusters); stalactites/stalagmites (low-poly meshes); boulders; vines; hanging roots; bones/skeleton decor; barrels/crates (breakable, loot).

### 8.6 World creation options
Seed (string/int), size preset, **mode** (Survival/Creative), **difficulty** (Explorer / Adventurer / Veteran / Master / Hardcore — §12.9), cheats toggle, biome frequency sliders (P2), “Blight spreads” toggle (P2), starting kit toggle, world name. World metadata: `world.meta.ron` (seed, size, version, created, playtime, difficulty, world phase, mod list).

### 8.7 World feature list
| ID | Feature | Pri | M | Acceptance criteria |
|---|---|---|---|---|
| WLD-001 | Deterministic `gen_chunk`, region-hashed features | P0 | M6 | Golden hash tests pass on Linux + Windows CI |
| WLD-002 | Surface terrain w/ 2 biomes (Verdant, Stonecrown) incl. arches | P0 | M6 | Visually matches Mockup B tone; 0 floating-garbage artifacts in 1000-seed fuzz |
| WLD-003 | Caves (cavern + tunnel + shaft) with connectivity guarantee | P0 | M6 | BFS from spawn reaches S2 floor in 100% of 1000 seeds |
| WLD-004 | Ore tables & veins, per-depth rarity | P0 | M6 | Statistical test: ore density within ±15 % of spec |
| WLD-005 | Strata 0–2 palette/lighting/spawn tables | P0 | M6–M7 | |
| WLD-006 | All 6 strata + blending | P1 | M11 | |
| WLD-007 | All surface biomes (excl. Skyreach/Blight) | P1 | M11 | Biome map plausibility + borders blended |
| WLD-008 | Structure placement system + templates + terrain adapt | P1 | M11 | No structure intersects water/lava or is buried unreachable in 1000 seeds |
| WLD-009 | Vegetation/props instancing + edit-delta for removals | P0 | M6 | Chopping a tree persists |
| WLD-010 | Liquid placement (ocean, lakes, lava) | P1 | M11 | |
| WLD-011 | World phases (§11.4) alter generation of *newly generated* chunks + convert existing ores | P2 | M14 | |
| WLD-012 | Skyreach islands | P2 | M14 | |
| WLD-013 | Blight spread simulation (cheap, chunk-local, rate-limited) | P2 | M14 | Max 1 ms/frame budget |
| WLD-014 | Worldgen preview CLI (`us_tools worldgen-preview --seed --png`) | P0 | M6 | Emits top-down + slice PNGs for agent inspection |
| WLD-015 | World creation UI + presets | P1 | M9 | |

---

## 9. Rendering, Lighting & Atmosphere

### 9.1 Pipeline summary
HDR forward(+) → depth/normal prepass → **cascaded shadow maps** (sun) → opaque terrain/blocks/props/creatures → transparent (ghosts, water, particles) → **volumetric fog + light shafts** → **bloom** → **tonemapping** (filmic) → color grading → **MSAA or SMAA** → UI. **Viewmodel** rendered in a separate pass (own FOV ≈ 60°, depth-cleared, wall-pushback; MOC-001).

### 9.2 Terrain & block materials (custom WGSL)
- Inputs per vertex: position, face normal (flat), material ids (2), weights (2), AO, packed light (sky+RGB), skin factor.
- Fragment: triplanar texture-array fetch (§5.5), dither blend, emission, apply light (sky × time-of-day tint + block RGB + dynamic lights), AO, fog.
- **Stable under edits**: UVs from world pos; no per-chunk UV resets.
- Grid overlay (BLD-002) and ore glint effects live in this shader via uniforms/flags.

### 9.3 Light model
**Voxel-grid light** (like classic sandbox games) + a **few dynamic lights**:
- `LightChunk`: `u16` per cell = `sky:4 | r:4 | g:4 | b:4`.
- **Propagation**: BFS flood fill. Each step attenuates by 1; opaque cells block; `light_filter` on blocks absorbs more; terrain cell counts as solid if its center sample < 0. **Sky light** propagates straight down without attenuation, horizontally with −1.
- **Incremental updates** on edits (add/remove light BFS), **budget ≤ 1.5 ms/frame**, spreading work across frames; light jobs run in `us_sim` and are applied on main thread.
- Vertices sample light by trilinear interpolation of the 8 surrounding cells → **smooth lighting**; flat-shaded facets still look right because only the normal is flat.
- **Emitters**: torches, lanterns, glow crystals/mushrooms, lava, enchanted gear. Colors come from `BlockDef.emits_light` / `MaterialDef.emission` (MOC-004).
- **Dynamic lights**: player held torch, projectiles, boss attacks, explosions → Bevy point lights, **max 8 shadowless + 2 shadowed** near camera *(tunable)*, clustered forward.
- **Ambient**: underground ambient = near-zero (stratum tint minimal); surface ambient from sky color and time-of-day.

### 9.4 Ambient occlusion
- **Baked per-vertex AO** for terrain (from SDF neighbors) and blocks (classic 4-corner AO) — P0.
- **SSAO** (GTAO) as a quality-preset option — P1.

### 9.5 Emissives, bloom, grading
Emissive texels (mushroom caps, crystal faces, rune glow) feed bloom. Per-stratum color grading LUT (blend by player Y) (P1). Damage vignette, low-health desaturation pulse, underwater tint.

### 9.6 Fog & atmosphere
- **Distance fog** per stratum/biome; **height fog** on surface valleys.
- **Volumetric light shafts** from the sun through foliage/arches (MOC-005) — P1 (use Bevy’s volumetric fog/light if suitable; else in-house raymarch at ¼-res).
- **Cave haze** (low-density volumetric tinted by stratum) to separate depth layers in dark caverns (MOC-004).
- **Sky**: procedural gradient + sun/moon/stars, clouds (low-poly cloud meshes or noise cards), aurora (Frostmarch, P3).

### 9.7 Particles & VFX (MOC-005, MOC-018)
Required families: dig debris, dust, sparks/embers (torches, forge, combat), floating leaves, spores (Glowdeep), fireflies (night surface), rain/snow, water splash/ripples, lava glow/embers, magic trails, hit flashes, damage numbers (billboards), pickup sparkle, ore glint, rune glow. Prefer GPU particles; cap by quality preset; **never exceed 3000 live particles** at Medium *(tunable)*.

### 9.8 Vegetation & wind
Grass/leaf sway via vertex shader wind (world-space noise); player/mob push-through bend (P2). Trees static meshes with LOD billboard impostors at distance (P1).

### 9.9 LOD & culling (see §19)
- Frustum culling per chunk; **cave occlusion culling** (chunk-face visibility graph flood from camera chunk) so closed caverns don’t render (P1).
- Terrain far-LOD with skirts (TER-015). Props/vegetation distance fade and impostors.
- Creature LODs: 2 levels for complex mobs; animation tick-rate reduction at distance.

### 9.10 Rendering feature list
| ID | Feature | Pri | M | Acceptance criteria |
|---|---|---|---|---|
| RND-001 | Terrain material: triplanar tex-array + flat normals + materials | P0 | M1 | Matches MOC-002; texel density test scene shows uniform 16 px/m |
| RND-002 | Block material & mesh path | P0 | M4 | |
| RND-003 | Sun + cascaded shadows + time-of-day sky | P0 | M7 | Shadow acne/peter-panning free in scene checklist |
| RND-004 | Grid-light BFS (sky + RGB) incl. incremental updates | P0 | M7 | Placing/removing torch updates ≤ 2 frames; budget respected |
| RND-005 | Dynamic lights (held torch, projectiles) | P0 | M7 | Torch lights nearby walls; light follows player smoothly |
| RND-006 | Bloom + tonemap + emissive | P0 | M7 | |
| RND-007 | Volumetric fog + light shafts | P1 | M14 | Mockup-B-like god rays at golden hour; ≤ 1.5 ms @1080p Medium |
| RND-008 | Baked AO (terrain + blocks); SSAO option | P0 / P1 | M7 / M14 | |
| RND-009 | Viewmodel pass w/ sway, wall pushback | P0 | M3 | No wall clipping at any FOV |
| RND-010 | Particle systems (families in §9.7) | P0 core / P1 full | M2–M14 | |
| RND-011 | Vegetation instancing + wind | P0 | M6 | ≥ 50k grass instances at Medium within budget |
| RND-012 | Cave occlusion culling | P1 | M15 | ≥ 30 % frame-time reduction in cave benchmark |
| RND-013 | Quality presets (Low/Med/High/Ultra) + render-scale | P1 | M14 | Each preset has documented toggles |
| RND-014 | Per-stratum color grading + fog profiles | P1 | M11 | |
| RND-015 | Photo mode (free camera, DOF, filters, hide HUD) | P2 | M14 | |
| RND-016 | Third-person camera option | P2 | M14 | Viewmodel hides; character model renders |

---

## 10. Fluids

### 10.1 Model
Level-based **cellular automaton** on the cell lattice (separate sparse layer, not in the SDF): `FluidCell { kind: u8 (water|lava|…), level: u8 (0..=8) }`. A cell is *fluid-passable* if its terrain center sample is air and no solid block occupies it. Partial slopes: fluid sits in air cells only.

### 10.2 Rules (10 Hz tick, active-set only, ms-budgeted)
1. **Fall**: if cell below is passable & not full → transfer as much as fits.
2. **Equalize** with 4 horizontal neighbors (≤ 1-level steps, deterministic ordering to avoid jitter).
3. **Sources**: ocean/lake/spring cells are infinite sources (level pinned at 8).
4. **Interaction**: lava + water → basalt/obsidian (terrain-fill op on the cell); fluids ignite/extinguish flammables; digging below a lake lets water flow in naturally.
5. **Budget**: ≤ `fluid_budget_ms` (default 1.5) per tick; spill remaining to next tick; cap active cells per chunk.
6. **Persistence**: store only chunks with non-source fluid state differing from generated state.

### 10.3 Rendering & gameplay
Per-chunk fluid surface mesh (flat low-poly, animated vertex waves, soft edges, fresnel, depth tint, foam at shorelines P2). Swimming (MOV-004), buoyancy for items, breath meter, lava damage + burning, water extinguishes burning, wet status (P2), underwater fog/tint/caustics (P2).

### 10.4 Fluid feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| FLD-001 | Fluid CA (water/lava) w/ budget + active sets | P1 | M11 | 100×100 flood-fill test stabilizes without jitter; budget respected |
| FLD-002 | Fluid mesh + rendering | P1 | M11 | |
| FLD-003 | Swimming, breath, lava damage | P1 | M11 | |
| FLD-004 | Fluid persistence + MP sync (delta only) | P1 | M13 | |
| FLD-005 | Water/lava interaction → basalt/obsidian | P1 | M11 | |
| FLD-006 | Pumps/buckets (carry fluid) | P2 | M14 | |
| FLD-007 | Waterfalls/streams generation | P2 | M14 | |

---

## 11. Time, Weather, Events & World Phases

### 11.1 Day/night
24-min day *(tunable)*: dawn 15 %, day 45 %, dusk 15 %, night 25 %. Sun/moon arc, sky color curve, ambient tint, star visibility, music cues. Beds skip night if all players sleep (MP: majority).

### 11.2 Weather (per-biome)
Clear, overcast, rain, storm (lightning flashes, thunder SFX), fog, snowfall, sandstorm (P2). Weather affects visibility, audio, ambient particles, spawn tables; rain extinguishes torches placed outdoors? **No** (annoying) — instead rain dims exposed campfires only (P3).

### 11.3 Events (hand-authored data: `assets/data/events/*.ron`)
| ID | Event | Pri | Description |
|---|---|---|---|
| EVT-001 | **Gloaming** (night event) | P1 | Increased spawns + elite variants; better drops; warns player at dusk |
| EVT-002 | **Warband raid** (goblin-like invasion w/ banner leader) | P1 | Waves toward player’s base; defend; loot |
| EVT-003 | **Starfall** (meteor impacts) | P1 | Meteor crater via terrain ops; deposits rare ore; player can see trajectory |
| EVT-004 | **Spore Bloom** (Glowdeep) | P2 | Temporary glowing spores, rare harvest, aggressive sporelings |
| EVT-005 | **Cave-in rumble** (cosmetic quake + falling debris, no terrain loss) | P2 | Pure feel |
| EVT-006 | **Wandering merchant** | P1 | Roaming trader with rotating stock |
| EVT-007 | **Rare spawns / mini-bosses** | P1 | Telegraphed with sound + map ping |
Events are server-driven, deterministic from `(world_seed, day_index)` for scheduling; clients receive start/stop messages.

### 11.4 World phases (post-boss escalation) — P2 (cut-able)
Defeating the **Citadel Warden** triggers **Phase II: “The Deepening”**: new ore generation in newly created chunks and **conversion of select existing ore**, new enemy tiers per stratum, new events, NPC unlocks, new crafting stations; map toast + cinematic camera ping. Defeating the **Cinder Wyrm** triggers **Phase III**. Phase stored in `world.meta.ron`.

---

## 12. Player, Items, Inventory & Crafting

### 12.1 Character
First-person adventurer, **no classes**; power comes from gear, crystals, accessories, runes. Derived stats: `max_health, max_mana, defense, move_speed_mult, mining_power, mining_radius, melee/ranged/magic damage mults, crit_chance, luck, light_radius_bonus, fall_resist, breath_max, pickup_range`. Cosmetic options (P1): body type/skin tone/hair/colors (visible in MP and 3rd person).

### 12.2 Health & mana (resolves A-2, A-3)
| Stat | Rule *(all tunable in `balance/player.ron`)* |
|---|---|
| Health unit | **1 half-heart = 10 HP**; 1 heart = 20 HP |
| Start / max | **5 hearts (100 HP)** → up to **15** via **Life Crystals**, **20** via **Life Fruit** (post-Phase II, P2) |
| Regen | After 5 s without damage: +1 half-heart / 4 s (scales with buffs/accessories); “resting” (sitting/bed) ×2 |
| Mana unit | **1 star = 20 mana** |
| Start / max | **2 stars** → **10** via **Mana Crystals** |
| Mana regen | 1 star / 6 s idle; slower while casting; potions restore |
| Invulnerability | 0.6 s i-frames after taking damage (flash viewmodel/HUD) |
| HUD | Half-heart granularity, stars stacked below hearts (MOC-006); rows wrap at 10 icons; numerical readout optional |

### 12.3 Inventory model
| Section | Slots | Notes |
|---|---|---|
| Hotbar | **10** (keys `1`–`0`, scroll) | Selected slot highlight; zero-count “ghost” of last item shown dimmed (MOC-007) |
| Main pack | **30** | Grid; backpack upgrades +10 each (max +4, P2) |
| Ammo | 4 | Auto-selected by weapon |
| Currency | 3 denominations auto-merge (**Bit = 1, Mark = 100, Crown = 10,000** *(working names)*) | Separate pouch |
| Equipment | Armor ×3 (helm/chest/greaves), Accessory ×5, **Off-hand ×1**, Vanity ×3 (P2), Dye (P3) | |
| **Strongbox** | Personal storage shared by all placed Strongbox blocks (per player) | P1 |
| Chests | 40 slots (wood) / 60 (iron) / 80 (rune) *(tunable)* | Block entities |
Default max stack 99 (materials 999); tools/weapons/armor 1; potions 30 *(per-item `max_stack`)*.

### 12.4 Item definition (data: `assets/data/items/*.ron`)
```ron
ItemDef(
  id: "us:copper_pickaxe", name_key: "item.copper_pickaxe", desc_key: "item.copper_pickaxe.desc",
  kind: Tool( ToolDef( class: Pickaxe, tier: 2, power: 1.6, brush_radius: 1.4, reach: 4.5, swing_time: 0.45 ) ),
  rarity: Common, max_stack: 1, value: 220,
  model: "models/items/pickaxe_copper.glb", icon: "icons/pickaxe_copper",
  sockets: 0, tags: ["metal","tool"], sound_set: "tool_metal",
)
```
Item **kinds**: `Tool` (pickaxe, axe, hammer, trowel, shovel), `Weapon` (melee/ranged/magic/thrown), `Armor`, `Accessory`, `Consumable` (potion/food/scroll), `Placeable` (block/furniture/station/torch), `TerrainFill`, `Material`, `Ammo`, `Key` (keys, boss summons), `Currency`, `Misc` (lore, maps).
**Rarity** colors: Common (white) → Uncommon (green) → Rare (blue) → Epic (purple) → Legendary (amber) → Quest (cyan).
**Prefixes/reforging** (P2): random modifiers on craft/loot (e.g., *Keen*, *Heavy*); reroll at a Tinker bench for currency.

**Mockup-derived starter items (P0)**: `wood_pickaxe, wood_sword (runed variant later), hammer, wood_axe (needed to harvest wood), wood_plank_block, stone_block, torch, healing_potion, builders_trowel`.

### 12.5 Tool tiers & materials (binding ladder; names are working)
| Tier | Material set | Pickaxe tier digs (min_tool_tier) | Typical location |
|---|---|---|---|
| 0 | Hands | dirt, clay, sand, gravel, snow | Anywhere |
| 1 | Wood / flint | `stone`, `copper_ore` | S0–S1 |
| 2 | Copper | `iron_ore`, `dense_stone`, `ruin_stone` | S1–S2 |
| 3 | Iron | `silverite_ore` | S2–S3 |
| 4 | Silverite | `aurum_ore`, `basalt` | S3–S4 |
| 5 | Aurum | `magmite_ore`, `obsidian` | S4 |
| 6 | Magmite → Voidsteel | `voidrock`, `voidsteel_ore` | S5 |
Gear ladders (weapons/armor/accessories) follow the same tiers. Each tier introduces **one new station or mechanic** (e.g., T2 Furnace→Anvil, T3 Alchemy Bench, T4 Rune Table, T5 Magma Forge, T6 Void Altar).

### 12.6 Equipment, off-hand, accessories
- **Armor sets** (3 pieces) with **set bonus** (e.g., +mining radius, +fire resist, +mana regen).
- **Accessories** (5 slots): data-driven stat/ability modifiers (double jump, dash, fall-resist, magnet range, spelunker, night vision, water walking P2…).
- **Off-hand** (MOC-001): `Torch` (auto-light, held torch lights surroundings), `Lantern` (brighter, drains oil P3), `Shield` (block: reduces damage in 120° front arc; P2), `Tome` (magic focus). Visible in viewmodel left hand.

### 12.7 Crafting
```ron
RecipeDef( id: "us:copper_pickaxe", station: Some("us:anvil"),
  ingredients: [("us:copper_bar", 12), ("us:wood", 4)],
  result: ("us:copper_pickaxe", 1), craft_time: 1.5, unlock: Auto )
```
- **Stations are blocks** with tag `station:<id>`; recipes require the station block within **4 m** (tier-up stations satisfy lower-tier needs, e.g., “Magma Forge” counts as Furnace).
- **Craft from nearby chests** (radius **12 m**, toggle; P0 as stretch, P1 required) — see QOL-010.
- **UI**: category tabs, search box, “craftable now” filter, **ghost recipes** (missing ingredients highlighted), craft ×1/×5/Max (`Shift`/`Ctrl`), **pin recipe** to HUD tracker (shows have/need incl. chests), recipe **tooltips w/ stat comparison**.
- Stations: Workbench, Furnace, Anvil, Alchemy Bench, Cooking Pot, Sewing Table (armor), **Rune Table** (§12.8), **Magma Forge**, **Void Altar**, Tinker Bench (reforge).
- **Potions & food**: Healing/Mana potions (**red/blue**, A-4), buffs (mining speed, regen, night vision, spelunker, swiftness, ironskin), **potion sickness** cooldown 45 s on Healing *(tunable)*; cooked food gives long-duration “Well Fed” buffs.

### 12.8 Runes & enchanting (MOC-016) — P2
Weapons/tools/armor have **rune sockets** (0–3 by rarity). Runes are items with modifiers (e.g., *Ember Rune*: +fire damage & burn chance; *Delve Rune*: +mining radius; *Leech Rune*: lifesteal). Crafted/slotted at the **Rune Table**; slotted runes **glow on the model** (emissive rune texture mask, color by rune school).

### 12.9 Difficulty & death
| Mode | Damage taken | Death penalty | Notes |
|---|---|---|---|
| **Explorer** | ×0.6 | Keep everything; respawn instantly | Relaxed |
| **Adventurer** (default) | ×1.0 | Drop **10 % of currency** at grave | |
| **Veteran** | ×1.5 | Drop 25 % currency; debuff “Weakened” 60 s | More HP on enemies; better loot |
| **Master** | ×2.0 | Drop 50 % currency | Boss phases harder |
| **Hardcore** (flag) | per above | **Permadeath** (character becomes spectator / deleted per setting) | |
- **Grave marker** entity at death spot with dropped currency; map death-pin; retrievable.
- **Respawn**: bed > waystone > world spawn. 5 s respawn timer (skippable; MP: others can revive in Co-op “Downed” state, P2).

### 12.10 Item/inventory/crafting feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| ITM-001 | Data-driven item registry + validator CLI | P0 | M5 | `us_tools validate-content` fails CI on broken refs |
| ITM-002 | Tools: pickaxe/axe/hammer/trowel w/ tiers & power | P0 | M5 | |
| ITM-003 | Weapons: melee (sword, hammer-weapon, spear), ranged (bow, crossbow), magic (staff, wand), thrown (bomb, knife) | P1 | M8–M12 | Each category ≥ 4 items by 1.0 |
| ITM-004 | Armor sets w/ bonuses (≥ 6 sets) | P1 | M12 | Set bonus applied/removed correctly |
| ITM-005 | Accessories (≥ 25) | P1 | M12 | |
| ITM-006 | Consumables: potions, food, scrolls | P1 | M8 | |
| ITM-007 | Prefix/reforge system | P2 | M14 | |
| ITM-008 | Runes & Rune Table | P2 | M14 | |
| INV-001 | Hotbar/pack/equipment/ammo/currency UI & logic | P0 | M5 | Drag, split, merge, swap, stack rules unit-tested |
| INV-002 | Chests (block entities) w/ UI | P0 | M5 | |
| INV-003 | Strongbox | P1 | M11 | Shared across placements |
| INV-004 | Backpack upgrades | P2 | M14 | |
| CRF-001 | Recipe registry + station detection | P0 | M5 | |
| CRF-002 | Crafting UI (tabs, search, filters, craft-from-chests, pin) | P0 basic / P1 full | M5 / M14 | |
| CRF-003 | All stations in §12.7 | P1 | M11–M12 | |
| CRF-004 | Buff/debuff framework | P1 | M8 | Stacking/refresh rules unit-tested |
| DTH-001 | Death, grave, respawn, difficulty modes | P1 | M9 | |

---

## 13. Combat, Enemies, Bosses & AI

### 13.1 Damage model *(tunable in `balance/combat.ron`)*
```
raw    = weapon_damage × stat_mults × (crit ? crit_mult : 1) × rand(0.95..1.05)   // RNG seeded per-hit by server
dmg    = max(1, raw × (1 − defense / (defense + K)))                               // K ≈ 60
final  = dmg × (1 − resist[type])                                                  // type ∈ {slash,pierce,blunt,fire,frost,poison,arcane,void}
```
Knockback scales with weapon + target `knockback_resist`. Bosses have `stagger` meters (break → brief vulnerability). Damage numbers (toggle) colored by type/crit.

### 13.2 Player combat (first-person)
- **Melee**: swing arc = capsule sweep along animated weapon path during an **active window** (e.g., 0.12–0.28 s of a 0.45 s swing). One hit per target per swing. **Hitstop** 40 ms *(setting-scalable)*, viewmodel recoil, SFX per material hit.
- **Ranged**: bows (draw time → power), crossbows (reload), ammo types (arrow variants: flint, iron, fire, rune). Projectile gravity/drag; headshot multiplier ×1.5 (P2).
- **Magic**: staffs/wands cost mana; hold-to-channel and tap-to-cast variants; low-mana → weak “fizzle” feedback.
- **Thrown**: bombs (terrain crater, TER-018), knives, flasks.
- **Sprint-attack**, **jump-attack**, **charged swing** (P2). **Blocking** with shield (P2).
- **Targeting assist**: soft magnetism for controller (setting); crosshair hit-marker.

### 13.3 Enemy roster (working names; ★ = mockup-derived)
| Enemy | Stratum | Role | Behavior | Notes |
|---|---|---|---|---|
| ★ **Cave Skitter** | S1–S2 | Swarm melee | Packs of 4–12, flow-field chase, jump-bite, flee at low pack count | Glowing eyes; light-shy (avoid ≥ light 8) → torches deter *(tunable)* |
| ★ **Runebound Brute** | S2 | Elite bruiser | Slow wind-up slams, rune-charged shockwave, rock throw; **staggerable** | Glowing cyan runes light the cave; mini-boss variant |
| ★ **Bone Shieldbearer** | S3 (+ ruins) | Tank melee | Raised shield blocks frontal 120°; bash; vulnerable from sides/behind | Pairs with archers |
| **Bone Archer** | S3 | Ranged | Keeps distance, arcs arrows, retreats | |
| ★ **Fiendling Imp** | S0 (ruins) / S3 | Flyer | Hover-strafe, fire darts, dive | Red winged |
| ★ **Moss Goblin Brute** | S0 | Melee | Charge attacks, club | Green; warband leader variants |
| ★ **Gargoyle Sentinel** | S0 gate / S3 | Ambush | Dormant statue → animates on approach; heavy drop-attack | Drops stone shards |
| **Gel** (slime analog) | S0–S1 | Filler | Hop, split on death | Color by biome |
| **Cave Flitter** (bat analog) | S1–S2 | Flyer | Erratic swoop | Dislikes light |
| **Glowcap Sporeling** | S2 | Support | Releases spore clouds (slow/poison) | Harvestable lights |
| **Burrower** | S1–S4 | Tunneler (P2) | Digs through terrain via server `EditOp`s | TER-017 |
| **Cinder Elemental** | S4 | Fire caster | Lava-walk, fireballs | Fire immune |
| **Magma Crawler** | S4 | Melee | Leaves lava trail briefly | |
| **Void Stalker** | S5 | Stealth | Fades in darkness; punished by light | |
| **Mimic Chest** | All | Ambusher | Looks like chest | Great loot |
| **Passive fauna** (deer, rabbits, birds, fish, cave glowbugs) | S0–S2 | Ambient | Flee/wander; harvestable | P1 |

**Enemy definition (data: `assets/data/enemies/*.ron`)**
```ron
EnemyDef(
  id: "us:cave_skitter", name_key: "enemy.cave_skitter",
  model: "models/enemies/cave_skitter.glb", capsule: (radius: 0.35, height: 0.5),
  health: 24, defense: 2, knockback_resist: 0.1, resist: { Poison: 0.5 },
  locomotion: Crawler( speed: 4.8, jump: 1.2, wall_climb: false ),
  ai: Swarm( aggro_range: 16.0, leash: 40.0, pack_size: (4, 10), flee_when_pack_below: 2, avoid_light_above: 8 ),
  attacks: [ Melee( name: "bite", damage: 6, range: 1.0, windup: 0.25, cooldown: 0.9, damage_type: Pierce ) ],
  loot: "us:skitter_loot", xp: None, spawn_weight: 1.0,
  sounds: "enemy_skitter", bestiary: ( order: 12, entry_key: "bestiary.cave_skitter" ),
)
```

### 13.4 AI & navigation on deformable terrain
**Architecture per mob**: `Perception → Brain (utility AI + FSM; BT allowed) → Locomotion → Animation`.
- **Perception**: vision cone + LOS via SDF/block raycast; **hearing** from noise events (digging, combat, explosions) with radius & occlusion falloff; memory of last-known position (5–10 s).
- **Navigation graph (`us_sim::nav`)**:
  - Nodes: lattice cells where the mob’s capsule fits with ground within 0.3 m below.
  - Edges: walk, step-up ≤ 0.6 m, jump gap ≤ per-mob, drop ≤ 4 m (fall-damage-aware), door pass, ladder/rope.
  - Built **per chunk, async** from a snapshot (like meshing). `NAV_DIRTY` set on terrain/block edits; **paths are revalidated** on next repath tick; mobs hitting stale paths trigger a repath, not a crash.
  - **Hierarchical**: chunk-portal graph for long routes + local A*; path smoothing by capsule sweeps (string-pulling).
  - **Flow fields** for swarms (≥ 8 agents → shared region distance field to target, updated 2 Hz, cached per chunk region).
  - **Flyers**: coarse 2 m 3D grid A*.
  - **Fallback**: steer toward target with obstacle probes + stuck detection (≥ 2 s) → alternate path / give up (leash).
  - **Budget**: ≤ 2 ms/frame total for pathfinding; request queue with priority by distance to player.
- **Group behavior**: pack alert radius 12 m, role slots (front/flank) to avoid pile-ups, separation steering.
- **Telegraphs**: every dangerous attack has a readable windup (animation + audio cue + optional ground decal for AoE).

### 13.5 Spawn director
- Per-player **threat budget** accrues over time scaled by difficulty/depth/events; spawning consumes budget by enemy `cost`.
- Candidate positions sampled **24–48 m** from the player, **outside LOS**, on nav nodes, matching table conditions (stratum, biome, time, light level, event, phase).
- **Caps**: 30 hostile mobs per player *(tunable)*, global cap by server; **despawn** > 80 m & not in LOS after 20 s.
- **Housing safety**: no spawns inside valid NPC housing; **Cave light suppression**: cave spawns require cell light ≤ threshold (torches deter spawns around bases).
- Spawn *tables* data: `assets/data/spawns/*.ron` (weights + conditions).

### 13.6 Bosses (data-driven framework)
`BossDef { phases: [...hp_threshold, attack_set, arena_effects], attacks: [timeline scripts], summon: Item|Natural, arena: StructureRef, barrier: bool, enrage_rules, loot_table, music, intro_cinematic }`
- **Boss bar** top-center (name, HP, phase pips) (UI-xxx).
- **Arena**: boss anchor in structure; optional **barrier** closes on engage; leaving the arena triggers **enrage/heal** after 10 s.
- **MP scaling**: HP × (1 + 0.6·(n−1)) *(tunable)*; personal loot.
- **Telegraphs & fairness**: no unavoidable damage; i-frames respected; **practice dummy** at Tinker bench to measure DPS (P2).

| Boss | Where | Signature mechanics (design intent) | Reward / unlock |
|---|---|---|---|
| **Brood Queen** | S1 Spider Nest | Summons skitter swarms; web-line traps slow player; ceiling drop attack; tunnels in walls (terrain-safe) | Spider-silk accessories; unlocks grapple blueprint; Nurse NPC condition |
| **Runebound Colossus** | S2 Glowdeep | Rune-pillar puzzle phase (shatter glowing runes to stagger); ground-shock rings; rock hurl | Rune Table blueprint; Rune items; Mana Crystal |
| **Citadel Warden** | S0 Citadel → S3 | Gargoyle wing phase (aerial dives), then armored sentinel phase w/ shield-bash & chains | **World Phase II**; Aurum access path; Arcanist NPC |
| **Cinder Wyrm** | S4 | Lava-surfing serpent, fire breath cones, burrows through basalt (carves tunnels) | Magma Forge upgrade; **Phase III** |
| **Heart of the Strata** | S5 Void Sanctum | Multi-phase final: darkness mechanic (stay near light), tendril adds, terrain-collapse arena ripples (cosmetic edits) | Ending sequence; Master loot; NG+ unlocks (P3) |
| **Optional**: Skyreach Storm Roc (P2), Mire Hag (P2), Frost Colossus (P2), Sandwurm (P2) | | | |
Mini-bosses: **Runebound Brute (elite)**, **Mimic**, **Warband Chieftain**.

### 13.7 Status effects *(buff/debuff framework; stack rules per effect)*
Burning (DoT), Poisoned (DoT), Bleeding, Chilled (slow), Frozen (stun, P2), Slowed, Weakened, Stunned, Wet (P2, amplifies frost/shock), **Spore-Dazed**, **Darkened** (S5 stress; cured by light), **Well Fed**, **Regen**, **Ironskin**, **Swiftness**, **Night Vision**, **Spelunker**, **Mining Haste**, **Shielded**.

### 13.8 Loot
- **Loot tables** (`assets/data/loot/*.ron`): weighted entries w/ conditions `{stratum, biome, phase, difficulty, event, min_luck}` and rolls; supports guaranteed + random + rare tiers; nested tables.
- **Chests**: tiered by stratum; some locked (keys from mini-bosses), trapped (P2), mimics.
- **MP**: **personal loot** by default for chests/bosses; optional shared.
- **Pity** counters for rare drops (P2).

### 13.9 Combat/enemy feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| COM-001 | Health/mana/damage pipeline + i-frames | P0 | M8 | Unit tests for damage math incl. resist/defense/crit |
| COM-002 | Melee sweep hit detection + hitstop + feedback | P0 | M8 | No double hits per swing; hits through thin walls prevented |
| COM-003 | Ranged projectiles vs SDF/blocks/entities | P1 | M8 | Arrows stick in terrain |
| COM-004 | Magic + mana | P1 | M8 | |
| COM-005 | Status effects framework | P1 | M8 | |
| COM-006 | Shield blocking | P2 | M14 | |
| ENM-001 | Mob controller (kinematic) shared by player/mobs | P0 | M8 | |
| ENM-002 | Nav graph async build + invalidation on edit | P0 | M8 | Edit a tunnel mid-chase → mob repaths within 1 s |
| ENM-003 | Flow-field swarm movement | P0 | M8 | 30 Skitters hold 60 FPS (AI ≤ 2 ms) |
| ENM-004 | Roster for slice: Skitter, Brute, Shieldbearer, Imp | P0 | M8–M10 | |
| ENM-005 | Full roster per §13.3 | P1 | M11–M12 | |
| ENM-006 | Spawn director + tables + caps | P0 | M8 | Spawn rate matches table within ±20 % over 10 min |
| ENM-007 | Bestiary (kill-tracking unlock; lore; drops) | P1 | M14 | |
| BOS-001 | Boss framework (phases, timelines, bar, arena, barrier) | P1 | M12 | Boss can be fully authored in data + small scripts |
| BOS-002 | Bosses 1–3 | P0 (1 in slice) / P1 | M10 / M12 | |
| BOS-003 | Bosses 4–5 + optional | P1 / P2 | M12 / M14 | |
| LOT-001 | Loot tables + conditions + personal loot | P0 / P1 | M8 / M13 | |

---

## 14. UI, HUD, Controls, QOL & Accessibility

### 14.1 HUD layout (mockup-derived; `—` = absent in mockups, added by this spec)
```
┌──────────────────────────────────────────────────────────────────────────────┐
│ ♥♥♥♥♥♥♥♥♥  (hearts, half-heart granularity)      [buff icons row]  — top-right│
│ ★★★★★      (mana stars)                    [  BOSS BAR + phase pips ] — top-ctr│
│ [1][2][3][4][5][6][7]… hotbar (10)         [compass strip]  — P2              │
│  1  (selected number under slot)                                              │
│ — breath bubbles (under stars, only underwater)                               │
│                                                                               │
│                              +  (crosshair)                                   │
│                          [E] Open Chest  (interaction prompt)                 │
│                                                                               │
│ — pickup log / chat (bottom-left)                    Terrain mode [V]         │
│        Grid mode: Active                              Grid mode   [G]         │
│        Placement [Stone Block]  (toast, bottom-ctr)   ┌─────────────┐         │
│                                                       │  MINIMAP    │ ornate  │
│                                                       │   (A) arrow │ frame   │
│                                                       └─────────────┘         │
│                                                       Strata III · −172 m     │
└──────────────────────────────────────────────────────────────────────────────┘
```
Rules: all HUD elements anchor to screen edges with safe-area margins; **integer-snapped pixel scaling** (UI scale 1×/2×/3× + fractional option); every element can be hidden/moved in a HUD editor (P2).

### 14.2 Hotbar (MOC-007)
10 slots, `1`–`0` + scroll + gamepad bumpers; selected slot has bright frame + slot number below; **count** bottom-right; **durability/cooldown overlay** (A-6); **ghost slot memory**: when stack hits 0 the slot keeps a dimmed icon & “0” so refills auto-restock (QOL-004); Lock (favorite) flag icon.

### 14.3 Screens & menus
Main menu (Continue, Play → Character select → World select/Create, Settings, Mods (P3), Credits, Quit) · Pause (Resume, Settings, Save, Quit to Menu; **SP pauses sim**, MP doesn’t) · Inventory/Equipment · Crafting · Map · Journal (quests, bestiary, lore, collection, achievements) · Chest/Strongbox · Shop/NPC dialog · Death screen · Photo mode · Dev console (`~`, Creative/cheats only).

### 14.4 Control schemes (defaults; **all rebindable**; mouse+KB and gamepad)
| Action | KB/Mouse | Gamepad |
|---|---|---|
| Move / Look | WASD / mouse | L stick / R stick |
| Jump / Sprint / Crouch | Space / Shift (hold or toggle) / Ctrl (hold or toggle) | A / L3 / B |
| Primary use / Secondary use | LMB / RMB | RT / LT |
| Hotbar select | 1–0, scroll | LB/RB, d-pad radial |
| Interact | E | X |
| Inventory / Crafting / Map / Journal | Tab / C / M / J | View/Menu/ radial |
| **Terrain Mode / Grid Mode** | **V / G** | d-pad up / d-pad down |
| Rotate / Flip / Shape (Grid) | R / F / C | RS click + bumpers |
| Quick-heal / Quick-mana / Quick-buff | H / N / B | radial |
| Place torch (auto) | T | Y |
| Drop / Sort / Quick-stack | Q / `Ctrl+Click` UI / `Shift+Click` | menu context |
| Camera toggle (1st/3rd, P2) | F5 | R3 |
| Photo mode / Screenshot | F6 / F12 | — |
| Debug overlays | F3 (+Shift variants) | — |
Gamepad specifics: **radial menus** (hotbar, build shapes, emotes), configurable deadzones/curves, aim assist (soft), full **controller-navigable UI** with focus graph and on-screen glyphs that switch automatically with last-used device.

### 14.5 QOL catalog (this is a **core differentiator**; implement by priority)
| ID | QOL feature | Pri | M |
|---|---|---|---|
| **Inventory & items** | | | |
| QOL-001 | **Auto-pickup magnet** with configurable range; items stack on pickup | P0 | M2 |
| QOL-002 | **Quick-stack to nearby chests** (button/hotkey; radius 12 m) | P1 | M11 |
| QOL-003 | **Sort** (by type/rarity/name/value) for pack & chests; sort lock for hotbar | P1 | M11 |
| QOL-004 | **Hotbar auto-restock** from pack when stack depletes (placeables/ammo/torches/potions) | P1 | M11 |
| QOL-005 | **Favorite/lock** items (can’t be dropped/sold/sorted/quick-stacked) | P1 | M11 |
| QOL-006 | `Shift`-click quick-move, `Ctrl`-click move-all-of-type, drag-split (`RMB`), `Alt` half-stack | P0 | M5 |
| QOL-007 | Item **compare tooltips** (Δ stats vs equipped) with colored arrows | P1 | M11 |
| QOL-008 | **Search box** in inventory/chests (type-ahead, highlight matches across nearby chests) | P2 | M14 |
| QOL-009 | **Trash slot** with undo; **auto-sell junk** by tag at shop | P2 | M14 |
| QOL-010 | **Craft from nearby chests**; crafting counts include chests; chest contents listed in tooltip | P1 | M11 |
| QOL-011 | **Recipe pin tracker** (HUD) incl. have/need/where-in-chests | P1 | M14 |
| QOL-012 | Craft **max** / ×5 shortcuts; **repeat last craft** hotkey | P1 | M11 |
| QOL-013 | **Quick-heal/mana/buff hotkeys** pick best potion automatically | P0 | M8 |
| QOL-014 | **Auto-equip better armor** prompt (optional); “new item!” marker until viewed | P2 | M14 |
| **Mining & terrain** | | | |
| QOL-020 | **Auto-select best tool** for target when `Alt` held (or setting) | P1 | M11 |
| QOL-021 | **Vein-mine / area mining** upgrades (accessory/rune): mine whole ore vein connected cells | P2 | M14 |
| QOL-022 | **Ore sparkle** + **spelunker** + minimap ore pings | P1 | M11 |
| QOL-023 | **Terrain lock** toggle (Grid/placement ignores terrain edits) to prevent accidental digging | P1 | M4 |
| QOL-024 | **Tunnel assist** (hold `Ctrl`: dig straight, level-aligned 2×2 tunnel) | P2 | M14 |
| QOL-025 | **Torch auto-place** hotkey (places on nearest valid surface along view); torch belt accessory | P0 | M7 |
| QOL-026 | **Rope/ladder auto-extend** | P2 | M14 |
| **Building** | | | |
| QOL-030 | **Undo/redo** placements (Survival limited; Creative unlimited-ish) | P1 | M5 |
| QOL-031 | **Drag line/rect/box** with cost preview (BLD-008) | P1 | M5 |
| QOL-032 | **Eyedropper**, **replace**, **plane lock**, **symmetry**, **blueprints** (BLD-009…016) | P1/P2 | M5–M14 |
| QOL-033 | **Housing validity overlay** (shows missing requirements) | P1 | M12 |
| QOL-034 | **Terrain-flatten foundation** helper (Grid Mode: auto-flatten under footprint) | P2 | M14 |
| QOL-035 | **Material-need summary** for selected region/blueprint | P2 | M14 |
| **Exploration & travel** | | | |
| QOL-040 | **Waystone fast-travel** network; map-click teleport; **Recall shard** to bed/spawn | P1 | M11 |
| QOL-041 | **Map pins/markers** (color/icon/label), share with party, auto-markers (chests found, ore, death) | P1 | M11 |
| QOL-042 | **Depth/strata indicator** + coordinates (toggle) | P0 | M7 |
| QOL-043 | **Compass strip** with pinned markers | P2 | M14 |
| QOL-044 | **Pause-able single player**; **autosave** + manual saves + **rolling backups** | P0 | M9 |
| QOL-045 | **Fall-warning** hint when about to take big damage (optional) | P3 | post |
| **Combat** | | | |
| QOL-050 | **Damage numbers**, **hit marker**, **enemy health bars** (toggle, boss always) | P0 | M8 |
| QOL-051 | **Bestiary tooltips** show weaknesses after N kills | P2 | M14 |
| QOL-052 | **Respawn at last waystone/bed** option; **grave marker + map ping** | P1 | M9 |
| QOL-053 | **Practice dummy** | P2 | M14 |
| **UI/UX** | | | |
| QOL-060 | **UI scale**, HUD opacity, element hide/move | P1 | M14 |
| QOL-061 | **Tooltips everywhere** (hover/gamepad focus) with keywords & icons | P1 | M5 |
| QOL-062 | **Controls reminders** (contextual first-time hints, dismissible) + **Tutorial/hints** system | P1 | M10 |
| QOL-063 | **Journal**: auto-logged quests, lore, tutorials re-readable | P1 | M12 |
| QOL-064 | **Inventory filter by category tabs**; **item “where to find”** hint from bestiary/recipes | P2 | M14 |
| QOL-065 | **Notification center** (non-modal toasts, throttled) | P1 | M9 |
| **World/Multiplayer** | | | |
| QOL-070 | **Join via code/Steam friend**, **drop-in/out** | P1 | M13 |
| QOL-071 | **Per-player settings** (difficulty modifiers per character, MP) | P2 | M14 |
| QOL-072 | **Sleep voting**; **ready-check** before boss spawn | P1 | M13 |
| QOL-073 | **Server admin UI** (kick/ban/permissions/rollback) | P1 | M13 |
| QOL-074 | **World backups & restore UI** | P1 | M9 |

### 14.6 Accessibility (P1 unless noted; design in from M5)
| ID | Feature |
|---|---|
| ACC-001 | Full **key rebinding** incl. hold/toggle for sprint, crouch, aim, interact; mouse button remap |
| ACC-002 | **Colorblind modes** (protan/deutan/tritan) applied to UI, ghost validity colors (cyan/red) get **shape/pattern cues** (✓/✗ + outline style) |
| ACC-003 | **Text scale** 75–200 %, high-contrast UI theme, dyslexia-friendly font option |
| ACC-004 | **Subtitles & sound captions** (directional indicators for off-screen threats) |
| ACC-005 | **Reduce motion**: head bob, screen shake, camera kick, hit-stop, FOV kick, flashing (**photosensitivity-safe** mode caps flash rate & intensity) |
| ACC-006 | **FOV slider 60–110**; motion-blur toggle (default off); sprint FOV kick toggle |
| ACC-007 | **Aim assist** & **auto-target ranged** (gamepad) toggles; **hold-to-click** assist |
| ACC-008 | **One-handed layout** preset (P2), **adjustable stick curves**, trigger thresholds |
| ACC-009 | **Narration/screen-reader hooks for menus** (P2) via accessibility tree where supported |
| ACC-010 | **Cave visibility assist**: optional ambient floor, brighter minimap, enemy outline pulses |
| ACC-011 | Difficulty assists: damage taken scale, longer i-frames, aim-forgiveness, slower enemy attack speed, mining speed ×, **No-fall-damage** toggle |
| ACC-012 | Pause anywhere in SP; adjustable **time-limited prompts** (extended QTE windows – there are none by default) |

### 14.7 UI technology rules
- Use Bevy UI for shipping HUD/menus; **no immediate-mode egui in shipping UI** (dev tools only).
- Data-driven UI **layout files** (RON) for HUD anchors; icons from an **atlas** baked by `us_tools bake-atlas`.
- All user-visible strings via **localization keys** (§20.5). **Never concatenate** sentence fragments.
- Navigation focus graph for controller; every interactive widget has a tooltip hook.

---

## 15. Maps & Navigation

### 15.1 Exploration data
`ExploredMap`: per-chunk bitset marking **explored 2×2×2-cell blocks** (16³ bits = 512 B/chunk). Updated server-side near each player (radius **24 m**, LOS-aware) and replicated to the player’s client; stored per character-per-world.

### 15.2 Full map (`M`)
Pan/zoom; **depth slice slider** (scrub Y) with strata bands labeled; surface top-down view; icons: waystones, NPC homes, chests (found), ore pings, bosses (after discovery), death pin, party members, custom pins; filter toggles; click-to-set waypoint; click waystone to fast-travel (QOL-040). Fog-of-war reveals gradually; cartography NPC sells map fragments (P2).

### 15.3 Minimap (MOC-008) — requirements
| Aspect | Spec |
|---|---|
| Frame | Square, ornate bronze/orange frame, ~160 px @1× scale, anchored bottom-right; toggle size S/M/L; hideable |
| Surface mode | **Top-down color render** of terrain materials/shading + structures/props (stylized), north-up (default) or camera-aligned (toggle) |
| Underground mode | **Horizontal depth-slice map** at player Y ±3 m: **walkable/open cells light-grey on near-black**, walls dark, unexplored hidden; shows vertical shafts with ▲▼ glyphs when openings above/below (matches Mockup A) |
| Auto-switch | Underground when sky-light at player < threshold or Y < surface-height − 6 m; manual override key |
| Markers | Player arrow (yellow), party, waystones, pins, ore pings (if spelunker), hostile pings for **boss** only |
| Zoom | `+`/`−` or scroll while hovered; 3 levels |
| Perf | Rendered by a **low-rate job** (≤ 10 Hz) into a texture; **no per-frame CPU readbacks**; ≤ 0.3 ms/frame |
| Persistence | Reads `ExploredMap`; unexplored cells never leak info |
| Label | Strata name + depth under frame (e.g., `Glowdeep · −96 m`) |

### 15.4 Waypoints, waystones & recall
- **Waystones** (structure/crafted late) register on touch; **fast-travel** costs a small consumable (Waystone Shard) or is free in Creative; cooldown 5 s *(tunable)*.
- **Recall Shard** (consumable) returns to bed/spawn after a 3 s channel (interrupted by damage).
- **Map markers** sync to party.

### 15.5 Map feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| MAP-001 | Explored bitset + replication | P0 | M9 | Round-trip persists; MP syncs deltas |
| MAP-002 | Minimap surface + underground slice modes | P0 | M7–M9 | Matches MOC-008; ≤ 0.3 ms/frame |
| MAP-003 | Full map screen w/ depth slider, pins | P1 | M11 | |
| MAP-004 | Waystones + fast travel + recall | P1 | M11 | |
| MAP-005 | Compass strip | P2 | M14 | |
| MAP-006 | Cartographer NPC + map fragments | P2 | M14 | |

---

## 16. NPCs, Housing, Economy, Quests & Meta-Progression

### 16.1 NPC roster *(working names; roles are the design intent)*
| NPC | Role | Move-in condition | Services |
|---|---|---|---|
| **Wayfarer** (guide) | Tutorial hints, recipe lookups | Present at start camp | “How do I…” menu showing recipes for held item |
| **Merchant** | General store | Player holds ≥ 50 Marks | Basic supplies, torches, potions, ammo |
| **Surgeon/Nurse** | Healing | After first Life Crystal | Pay to heal; remove debuffs |
| **Smith** | Metalwork | After copper/iron bars crafted | Sells/repairs?, reforge (P2), station upgrades |
| **Alchemist** | Potions | After Glowdeep visit | Ingredients, potion recipes |
| **Arcanist** | Runes | After Citadel Warden | Rune sales, Rune Table access |
| **Cartographer** | Maps | After 30 % map explored (P2) | Map fragments, markers |
| **Angler** (P2) | Fishing quests | | |
| **Wandering Merchant** | Roaming (EVT-006) | Event | Rotating stock |
| **Tinkerer** (P2) | Gadgets | After Brood Queen | Accessory combos |
Up to **8 housed NPCs** in 1.0. Names/personalities are placeholders for the owner.

### 16.2 Housing validity (BLD-013, QOL-033)
A **valid home** = enclosed volume (flood-fill within a **20–400 cell** bounding limit; terrain+blocks both count as walls), contains **a door (or sealed passage)**, **≥ 1 light source**, **a table and a seat or bed**, and ≥ 2 cells of headroom. Overlay shows failing checks (e.g., “No light”). NPCs path to their homes at night; enemies don’t spawn inside valid homes.
(P2) **Happiness**: biome preference + neighbor density modifies shop prices (−10 … +15 %).

### 16.3 Economy
Three currency denominations; item `value` drives vendor sell/buy (buy = 4× sell by default *(tunable)*); no player-trading inflation controls needed in co-op. Shops have fixed stock + progression unlocks + rotating items. **Sink**: healing, reforge, fast-travel shards, repairs (no durability in 1.0).

### 16.4 Quests, lore & journal
- **Hand-authored** quest data (RON): objectives (kill/collect/deliver/reach/build), rewards, prerequisites, NPC dialogue trees (localization keys). 20–30 quests in 1.0 *(tunable)*; P1.
- **Lore items** (journals, tablets) in structures; **Journal** screen (QOL-063) with unlock tracking; **Bestiary** and **Collection log** (items obtained).
- **Achievements** (Steam + local), ~60 at 1.0, including “first dig”, “reach each stratum”, “defeat each boss”, “build with 1000 blocks”, “survive Gloaming”, etc.
- **Narrative frame** (placeholder for owner): ancient builders raised the Citadel above older strata; the deeper you dig, the older the civilization. **Owner must approve lore before text is authored.**

### 16.5 NPC/economy feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| NPC-001 | NPC entity + dialogue UI + shop UI | P1 | M12 | |
| NPC-002 | Housing detection + overlay | P1 | M12 | Test fixtures for 20 room layouts pass/fail correctly |
| NPC-003 | NPC move-in conditions + daily schedules | P1 | M12 | |
| NPC-004 | 6 NPCs for 1.0 (+2 stretch) | P1 / P2 | M12 / M14 | |
| NPC-005 | Happiness/prices | P2 | M14 | |
| ECO-001 | Currency + vendor pricing | P1 | M12 | |
| QST-001 | Quest framework + journal | P1 | M12 | |
| QST-002 | ≥ 20 quests | P1 | M14 | |
| ACH-001 | Achievements (local + Steam) | P1 | M15 | |

---

## 17. Multiplayer (Co-op)

### 17.1 Model
**Server-authoritative**, **2–8 players**, listen-server (host plays) and **dedicated server** (`understrata --server`). SP uses an in-process local server (§4.1).
- Transport: reliable+unreliable channels over **UDP/QUIC** (`quinn`/`renet`-class). Encryption (TLS/QUIC) enabled by default.
- **Replication** of ECS entities via a replication crate or in-house; **interest management** by chunk radius (default 10 chunks).

### 17.2 Client prediction & reconciliation
- **Player movement**: client predicts at 60 Hz, server reconciles with sequence-numbered inputs; **mis-prediction smoothing** under 0.3 m.
- **Digging/building**: client shows **instant local preview** (predicted brush & ghost) but world state changes only on server ack (`LoggedOp seq`). If rejected, preview reverts with soft flash.
- **Combat**: server resolves hits with **lag compensation** (rewind hurtboxes ≤ 150 ms) for ranged/hitscan; melee uses server-side sweeps with client-predicted viewmodel.

### 17.3 World sync
- **Chunk streaming** by interest: server sends **palette+LZ4/zstd-compressed** chunk snapshots for chunks the client lacks, then **EditOp deltas** (with chunk `version`) for chunks it has. Clients **never regenerate** worldgen independently for authority (they may regenerate unedited chunks locally if `WORLDGEN_VERSION` matches — optimization P2 using hash verification).
- **Edit batching**: merge ops per tick; cap per-client op rate (e.g., 20 brush ops/s *(tunable)*); server **validates**: reach ≤ tool.reach + ε, LOS, cost in inventory, cooldown vs tool speed, protected-area permissions.
- **Bandwidth targets**: steady-state < 64 KB/s per client; join burst < 4 MB for default interest radius.

### 17.4 Rules & permissions
Host/admin roles (owner/admin/member/guest) with permission flags: build, dig, use chests, PvP, invite, kick/ban, change time/weather (cheats), rollback. **PvP** off by default; optional team PvP (P2). **Friendly-fire** setting for projectiles/explosions. **Personal loot** default.

### 17.5 Multiplayer feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| NET-001 | Local-server SP architecture (intent→authoritative→delta) | P0 | M3 | SP gameplay never mutates world outside server systems (lint/test enforced) |
| NET-002 | Protocol crate `us_net` w/ versioned messages | P0 | M3 | Round-trip tests; version mismatch rejected clearly |
| NET-003 | Player movement prediction/reconciliation | P1 | M13 | 150 ms RTT, 2 % loss playable; no rubber-band spam |
| NET-004 | Chunk/edit replication + interest mgmt | P1 | M13 | Two clients edit same chunk concurrently → identical hashes after quiesce |
| NET-005 | Entity replication (mobs, items, projectiles, fluids) | P1 | M13 | |
| NET-006 | Dedicated server, config, headless | P1 | M13 | Runs w/o GPU; `--server --port` flags |
| NET-007 | Lobby/join (IP, code, Steam) | P1 | M13 | |
| NET-008 | Admin tools, permissions, rollback | P1 | M13 | |
| NET-009 | Anti-cheat validation (server-side checks above) | P1 | M13 | Fuzz test: tampered ops rejected |
| NET-010 | Latency/loss simulator in dev builds | P0 | M3 | CLI flags `--net-lag-ms --net-loss-pct` |
| NET-011 | Voice chat | P3 | post | |
| NET-012 | Cross-play / Steam relay | P3 | post | |

---

## 18. Audio

### 18.1 Buses & mixing
`Master → {Music, SFX, Ambience, UI, Voice(P3)}`; sliders per bus; **ducking** (music under boss stingers; ambience under dialogue); **dynamic range** setting (Night mode compresses). Spatial 3D audio with distance attenuation, HRTF-lite panning (P2).

### 18.2 Adaptive music
Per-stratum **stem layers** (pad, rhythm, melody, danger) blended by: stratum/biome, time of day, combat intensity, boss state, safe-house. Stingers: discoveries, level-ups (crystals), boss intros. Main theme + 10–14 stratum/boss tracks for 1.0 *(tunable)*.

### 18.3 Environmental audio
- **Material-driven SFX**: footsteps, dig, place, impact per `sound_set` × surface material.
- **Cave reverb**: estimate enclosure by casting ~16 rays; map to reverb presets (cavern/tunnel/hall/open); smooth transitions.
- **Occlusion/obstruction**: cheap LOS raymarch on SDF/blocks → low-pass + volume reduction for sources behind terrain.
- **Ambient beds** per stratum/biome; positional ambience emitters (dripping, lava bubbling, wind through arches).
- **Enemy audio cues**: distinct, learnable idle/aggro/attack-windup sounds; **directional threat indicators** (ACC-004).

### 18.4 Audio feature list
| ID | Feature | Pri | M |
|---|---|---|---|
| AUD-001 | Bus mixer + settings + spatial SFX | P0 | M2 |
| AUD-002 | Material footstep/dig/place SFX sets | P0 | M3 |
| AUD-003 | Cave reverb + occlusion | P1 | M14 |
| AUD-004 | Adaptive music (stems) + boss music | P1 | M14 |
| AUD-005 | Ambient beds/emitters per stratum | P1 | M11 |
| AUD-006 | Dynamic range/night mode, per-category mute | P2 | M14 |
| AUD-007 | Subtitles/sound captions integration | P1 | M14 |

---

## 19. Performance Targets & Budgets

### 19.1 Hardware targets
| Tier | CPU | RAM | GPU | Target |
|---|---|---|---|---|
| **Minimum** | 4-core @ 3 GHz (≈ Ryzen 3 3100 / i5-8400) | 8 GB | GTX 1060 6GB / RX 580 / Intel Arc A380-class; Vulkan 1.2 / DX12 | **60 FPS @ 1080p, Low–Medium preset**, view dist 8 chunks |
| **Recommended** | 6-core | 16 GB | RTX 2060 / RX 6600 | 60 FPS @ 1440p High, view dist 12 chunks |
| **Steam Deck** (P2) | Van Gogh APU | 16 GB shared | integrated | 40–60 FPS @ 800p, Low–Medium |

### 19.2 Frame budget @ 60 FPS (16.6 ms) — Medium preset *(guides; enforce via benches)*
| Slice | Budget |
|---|---|
| Main-thread gameplay (input, ECS systems, UI) | ≤ 4.0 ms |
| Render extraction/submit (CPU) | ≤ 3.0 ms |
| GPU frame (1080p, min-spec) | ≤ 14.0 ms |
| AI + nav (aggregate) | ≤ 2.0 ms |
| Light updates | ≤ 1.5 ms |
| Fluid sim | ≤ 1.5 ms |
| Minimap job (main-thread part) | ≤ 0.3 ms |
| Audio mix (main part) | ≤ 0.5 ms |
Background threads: chunk gen, meshing, light BFS, nav build, compression, IO (N−1 workers; priority queues; **no unbounded queues**).

### 19.3 Streaming & latency targets
| Metric | Target |
|---|---|
| Player dig → mesh visible | **p95 < 100 ms**, p99 < 200 ms |
| Mesh time per 32³ chunk | **p95 < 3 ms** on min-spec CPU (single worker) |
| Chunk gen time | p95 < 8 ms per chunk (surface), < 5 ms (cave interiors) |
| World load → playable | < 15 s (SSD), Small world |
| Chunk hitch (any single-frame spike) | < 8 ms main-thread from chunk apply (use per-frame apply budget) |
| Autosave main-thread stall | < 5 ms |
| RAM at defaults | ≤ 3 GB RSS (Medium world, 12 chunk radius) |
| Save size (Small world, 20 h play) | ≤ 300 MB, typical < 100 MB |
| Triangles visible | ≤ 3 M (Medium) |
| Draw calls | ≤ 2500 (GPU-driven/indirect where possible; chunk meshes merged by material) |

### 19.4 Optimization levers (in order of preference)
Uniform-chunk collapse → frustum + **cave occlusion culling** → far-LOD + skirts → distance fade for props/particles → GPU instancing/indirect → reduce light update frequency → fluid active-set limits → AI LOD (tick rate by distance) → compress/palette chunk data → texture streaming/mips (KTX2/BC7).

### 19.5 Profiling hooks (required)
`tracing` spans around every major system, **Tracy**/Bevy diagnostics integration, **in-game F3 overlay** (FPS, frame ms breakdown, chunk counts, mesh queue, light queue, nav queue, memory, net stats), **benchmark scenes** (`--bench cave_dense`, `--bench surface_forest`, `--bench dig_stress`, `--bench swarm_30`) with CSV output for CI regression thresholds.

---

## 20. Persistence, Settings, Modding, Localization, Platforms & Release Ops

### 20.1 Save architecture
- **World** = `worldgen(seed, WORLDGEN_VERSION)` + **delta store**. Save only **edited chunks**, entities, fluids deltas, explored maps, world meta.
```
saves/<world_id>/
  world.meta.ron           # seed, size, versions, difficulty, phase, playtime, mods
  regions/r.<x>.<y>.<z>.usr# region = 8×8×8 chunks; header + per-chunk records; zstd/lz4; CRC32 per chunk
  entities.bin             # persistent entities (NPCs, placed block-entities, mobs w/ persist flag)
  players/<uuid>.ron|bin   # character data, inventory, map explored, stats (+ per-world player state)
  backups/ (rolling 3 + daily 7)
```
- **Atomic writes**: write temp + fsync + rename. Never leave partial files. **Corruption handling**: per-chunk CRC; on failure, regenerate unedited-region chunks from seed and log; edited-chunk corruption → restore from backup prompt.
- **Versioning**: `SAVE_VERSION` + migration functions chain (`migrate_v3_to_v4`). Tests load **fixture saves** from every prior version.
- **Autosave** every 120 s *(tunable)* + on quit/sleep; **Async**; main-thread stall budget §19.3.
- **Characters** are separate from worlds (Terraria-style) so characters can join other worlds (P1).

### 20.2 Settings (`settings.ron` in user config dir)
Graphics (preset + individual toggles, resolution/window mode, VSync, frame cap, render scale, view distance, shadows, AO, volumetrics, particles, FOV), Audio (buses), Controls (bindings + sensitivity + invert + deadzones), Gameplay (damage numbers, auto-pickup range, QOL toggles, difficulty assists), Accessibility (§14.6), Interface (UI scale, language, HUD toggles), Network. Changes apply live where possible; “Apply/Revert” with 15 s confirm for display mode.

### 20.3 Data-driven content & modding
- **All content in `assets/data/**` RON** with **stable string ids** (`namespace:name`). The validator (`us_tools validate-content`) checks references, duplicate ids, orphan assets, and balance sanity.
- **Mod loader (P2 data-only; P3 scripting)**: `mods/<modid>/mod.ron` (id, version, deps, load-order) overlays data/assets using the same schemas; conflict reporting; **mods disabled in Hardcore leaderboards**.
- **Scripting (P3)**: sandboxed **WASM** or Lua for events/boss attacks/items; capability-limited API; deterministic where server-affecting.
- **Workshop (P3)** via Steam.

### 20.4 Asset pipeline
| Asset | Source format | Runtime | Notes |
|---|---|---|---|
| Textures | PNG 16×16 tiles (+ UI sprites) | **KTX2** arrays/atlases via `us_tools bake-atlas` | Maintain 16 px/m; mip-gen with alpha-aware filter |
| Models (blocks/props/creatures) | `.glb` (Blender/Blockbench/MagicaVoxel→glTF) | glTF scenes | Faceted/voxel-ish low-poly; rigs ≤ 40 bones for mobs |
| Audio | WAV/FLAC masters | OGG Vorbis/Opus | Loudness-normalized (−16 LUFS music, −14 SFX peaks) |
| Data | RON | Parsed at startup, validated | Hot-reload in dev builds |
| Fonts | TTF/OTF | atlas | Pixel font + fallback with CJK coverage |
**Hot reload** for data/shaders/textures in dev builds (agents iterate faster).

### 20.5 Localization
All text via **keys** in `assets/lang/<locale>/*.ftl` (Fluent). Launch languages (P1): **en-US** (source) + P2: es, fr, de, pt-BR, ru, zh-CN, ja, ko. Support plural/gender via Fluent; **pseudo-localization** build flag to find overflow/hard-coded strings; font fallback chain; UI layouts tolerate +40 % text length; number/date formatting via locale.

### 20.6 Platforms & distribution
| Platform | Pri | Notes |
|---|---|---|
| Windows 10/11 x64 | P0 | Vulkan/DX12 via wgpu |
| **Linux x64 (Wayland + X11)** | P0/P1 | Vulkan; Steam Runtime compatible; AppImage/Flatpak (P2) |
| macOS (Apple Silicon) | P2 | Metal via wgpu; notarized |
| Steam Deck Verified | P2 | Controller-first UI, 800p presets |
| Steam integration (achievements, cloud saves, rich presence) | P1 | `steamworks` crate; stub for non-Steam builds |
| Consoles | — | Out of scope |

### 20.7 Telemetry, crash reporting, privacy
**Opt-in only** telemetry (no PII). **Crash handler**: panic hook writes `crash-<timestamp>.zip` (logs, settings, last frame stats, system info, world meta; **never** saves without consent) and offers to open the folder. Structured JSON logs (`tracing`) rotate (5 × 10 MB). Follow GDPR/CCPA-friendly practices; privacy policy in the store page.

### 20.8 Persistence/ops feature list
| ID | Feature | Pri | M | Acceptance |
|---|---|---|---|---|
| SAV-001 | Region file format + delta store + atomic writes | P0 | M9 | Kill -9 during save never corrupts; load next run succeeds |
| SAV-002 | Player/character save + character select | P0 | M9 | |
| SAV-003 | Version migration framework + fixtures | P0 | M9 | CI loads all fixture versions |
| SAV-004 | Autosave + rolling backups + restore UI | P0 / P1 | M9 / M14 | |
| SAV-005 | Corruption detection/recovery | P1 | M15 | Fuzzed corrupted files never panic |
| SET-001 | Settings system + presets + live apply | P0 | M2 | |
| MDK-001 | Content validator CLI | P0 | M5 | |
| MDK-002 | Data-only mod loader | P2 | M14 | |
| LOC-001 | Fluent localization + pseudo-loc | P1 | M5 | Zero hard-coded UI strings (lint) |
| PLT-001 | Windows + Linux CI builds & artifacts | P0 | M0 | |
| PLT-002 | Steamworks integration | P1 | M15 | |
| OPS-001 | Crash reporter + log rotation | P1 | M15 | |

---

## 21. Roadmap, Milestones & Exit Criteria

**Release gates**: **Vertical Slice / Internal Alpha = end of M10** · **Beta (feature-complete, P0+P1+most P2) = end of M14** · **1.0 RC = end of M15**.
Each milestone MUST end with: all listed feature IDs passing acceptance tests, CI green, updated `CHANGELOG.md`, a **recorded demo** (screenshots via harness §22.3), and no known P0 crashes.

| M | Name | Deliverables (feature IDs) | Exit criteria (binary, testable) |
|---|---|---|---|
| **M0** | **Foundations** | Workspace + crate layout (§4.2); toolchain pin; Bevy pin; CI (fmt/clippy/test, Win+Linux builds); logging; **headless harness skeleton** (DEV-003); content validator stub; `docs/*` scaffolding; PLT-001 | `cargo test --workspace` green on CI; `understrata --headless --frames 60` exits 0 on CI (software Vulkan); window opens locally |
| **M1** | **Terrain on screen** | TER-001/002/003/011(grass); RND-001; MOV-011 (fly cam); `us_tools worldgen-preview` stub; noise-based test terrain | 16×16 chunk area renders at 60 FPS (dev GPU); watertight mesh property tests; look-dev decision recorded (D-01); screenshot matches mockup *tone* |
| **M2** | **Dig & fill** | TER-004/005/007/008/009/012/019; QOL-001; AUD-001; SET-001; DEV-001 console; particles (dig) | Scripted dig of a 10 m tunnel edits only expected chunks; replaying op log reproduces hashes; edit→mesh p95 < 100 ms (bench) |
| **M3** | **Walk the world** | MOV-001/002/003; RND-009 viewmodel; NET-001/002/010; AUD-002; first-person camera | 10-min fuzz walker: never stuck > 1 s, never tunnels; SP mutations only via server systems (test) |
| **M4** | **Grid Mode & blocks** | BLD-001…006; TER-006; TER-020; QOL-023; block mesher; `EditMode` HUD legend/toast | Place/remove 1,000 blocks via script; ghost valid/invalid; mode toggle persists; overlay visible in screenshot |
| **M5** | **Items, inventory, crafting** | ITM-001/002; INV-001/002; CRF-001/002(basic); QOL-006/030/031/061; BLD-007…011/014; MOV-005; MDK-001; LOC-001; CRE-001 | Craft full wood→stone tool chain by script; chest roundtrip; validator catches broken refs; no hard-coded strings |
| **M6** | **Worldgen v1** | WLD-001…004/009/014; RND-011; vegetation instancing | 1,000-seed fuzz: no invalid chunks, spawn→S2 path exists; golden hashes stable across Win/Linux CI |
| **M7** | **Light & atmosphere** | RND-003…006/008(baked); WLD-005; QOL-025/042; MAP-002 (minimap v1); day/night | Torch placement relights ≤ 2 frames; Mockup-A-like cave screenshot produced via harness; light budget respected |
| **M8** | **Combat & enemies** | COM-001…005; ENM-001…004/006; LOT-001; QOL-013/050; CRF-004; ITM-006; MOV-007 | 30-Skitter swarm bench holds 60 FPS; mob repaths after tunnel edit ≤ 1 s; damage math unit tests |
| **M9** | **Persistence & menus** | SAV-001…004(basic); DTH-001; MAP-001; WLD-015; QOL-044/052/065/074 (basic); main/pause/death menus | Kill -9 during autosave never corrupts; load restores edits, inventory, explored map; fixture saves load |
| **M10** | **VERTICAL SLICE** | Polish pass on M1–M9; tutorial hints (QOL-062); **S0–S2 content**, Citadel gate (WLD-S01 partial), Outpost (WLD-S02), Glowcap Grove (WLD-S05), **Brood Queen** (BOS-002 slice) | A fresh player can: spawn → dig → craft → build house (Grid Mode) → descend to Glowdeep → beat a boss → save/quit/load. 30-min playtest has 0 crashes, ≥ 55 FPS on min-spec |
| **M11** | **World breadth** | WLD-006/007/008/010; FLD-001/002/003/005; TER-010/011(rest); MAP-003/004; QOL-002…005/007/010/012/020/022/040/041; AUD-005; RND-014; INV-003; CRF-003(part); ENM-005(part) | All 6 strata + 6 biomes generate in 1,000-seed fuzz; fluids stable; fast-travel works; structures valid |
| **M12** | **Progression breadth** | BOS-001/002/003; NPC-001…004; ECO-001; QST-001; ITM-003/004/005; CRF-003; MOV-006; TER-018; QOL-033/063; BLD-019 (doors); EVT-001…003/006/007; ENM-005 complete | Full tier ladder craftable; 3 bosses beatable via data; 6 NPCs move in/shop; events trigger deterministically |
| **M13** | **Multiplayer** | NET-003…009; FLD-004; TER-013; QOL-070/072/073; personal loot | 4-client soak (30 min, 150 ms RTT, 2 % loss): no desyncs; identical chunk hashes after quiesce; dedicated server headless |
| **M14** | **Polish, QOL, accessibility, P2s** | All **ACC-***; QOL P1/P2 remaining; RND-007/013/015/016; AUD-003/004/006/007; BLD-015…018/021; ITM-007/008; MDK-002; WLD-011…013; TER-014/016/017; BOS-003 (rest); NPC-005; QST-002; ENM-007; MAP-005/006; MOV-008/009; COM-006; FLD-006/007; SAV-004 (restore UI) | Beta feature-complete; accessibility checklist passes; Low/Med/High/Ultra presets validated; localization pseudo-loc passes |
| **M15** | **Optimization, balance, release** | TER-015; RND-012; SAV-005; PLT-002; OPS-001; ACH-001; balance passes; full QA; store/Steam prep; docs | §24 release checklist 100 % |
| **Post** | P3 backlog | Voice chat, workshop, scripting, NG+, strict build, mounts, etc. | |

### 21.1 Suggested vertical-slice demo script (what “done” looks like at M10)
1. Spawn in Verdant Reach at dawn; hint toasts teach movement/dig.
2. Chop tree (axe) → craft workbench → craft wood pickaxe/sword.
3. Toggle **Grid Mode (`G`)**; place a 6×6 wooden hut with door + torch → NPC Merchant moves in (if included early) or Housing overlay shows “valid”.
4. Dig into hillside; find copper → craft copper pickaxe at furnace+anvil.
5. Descend shaft to Rootwarren; fight Cave Skitters; light torches; minimap switches to underground slice.
6. Reach **Glowdeep**; see bioluminescent cavern; meet Runebound Brute (mini-boss).
7. Outpost loot → Life Crystal; fight **Brood Queen**; unlock recipe; save & quit; reload → everything persists.

---

## 22. Testing, Verification & the Agent Harness

> **Principle**: *If an agent can’t verify a change without a human looking at the screen, the change is not done.* Everything below exists so agents can run, observe, assert, and compare **headlessly**.

### 22.1 Test pyramid
| Layer | Scope | Tools | Run |
|---|---|---|---|
| Unit | Pure crates (`us_core`…`us_net`) | `cargo test` | every commit |
| Property | Meshing watertightness, edit-op determinism/commutativity where claimed, light conservation, inventory ops, DDA, nav graph invariants | `proptest` | every commit (bounded cases) + nightly (many cases) |
| Golden/Snapshot | Worldgen chunk hashes, structure placement lists, recipe graphs, loot distributions | `insta` | every commit |
| Integration (ECS) | Bevy `App` with minimal plugins, N ticks, assert world state | `cargo test` w/ headless Bevy | every commit |
| Scripted E2E (harness) | Full game headless: scripted player actions + assertions + screenshots | `understrata --headless --script` | PR + nightly |
| Visual regression | Screenshots at fixed seeds/cameras compared with tolerance | software Vulkan (lavapipe/llvmpipe) | nightly + milestone |
| Performance | `criterion` microbenches + `--bench` scene CSVs vs thresholds | | nightly + milestone |
| Soak / fuzz | 30–120 min random-walk/dig/place bots; 1000-seed worldgen fuzz; MP soak w/ lag sim | | nightly / weekly |
| Content validation | `us_tools validate-content` | | every commit |
| Save compatibility | Load fixture saves for all `SAVE_VERSION`s | | every commit |

### 22.2 Required invariants (must have tests)
- `gen_chunk(seed, c)` is **pure & deterministic** (run twice, different thread orders → same bytes).
- **Replay**: `world_after(seed, op_log) == world_after(seed, op_log)`; and `apply(inverse(op)) ≈ identity` for undo.
- **Mesh**: no cracks at chunk borders for random SDFs; vertex count within budget; flat normals unit length.
- **Light**: removing all emitters returns light to baseline; no light leaks through opaque terrain/blocks (sample tests).
- **Collision**: player never ends a tick overlapping solid by > ε; never tunnels at max speed (fall terminal velocity) through 1 m features.
- **Inventory**: item conservation (no dup/loss) under random drag/drop/craft sequences.
- **Nav**: after any terrain edit, no path references a now-solid node after revalidation.
- **Save**: save→load→save yields byte-identical region files (modulo timestamps) in fixtures.
- **Net**: op replication yields equal chunk hashes across N clients after quiescence.

### 22.3 Headless harness — CLI contract (**implement in M0; extend every milestone**)
```
understrata
  [--headless]                       # no window; software/null renderer allowed
  [--server]                         # dedicated server mode
  [--seed <u64|string>] [--size small|medium|large] [--world <path>] [--creative]
  [--script <file.ron>]              # scripted bot commands + assertions (below)
  [--ticks N | --frames N | --seconds S]   # termination conditions
  [--camera x,y,z,yaw,pitch]         # fixed camera for screenshots
  [--screenshot-at <tick>:<path> ...]# PNG capture at tick(s) (works with software Vulkan)
  [--dump-chunk x,y,z --out <path>]  # raw + JSON debug dump (sdf/mat/blocks/light/nav)
  [--bench <name> --bench-out <csv>] # cave_dense | surface_forest | dig_stress | swarm_30 | stream_fast
  [--log-json <path>] [--log-level info|debug|trace] [--assert-no-warnings]
  [--net-lag-ms N] [--net-loss-pct P] [--net-jitter-ms J]
  [--exit-code-on-panic]             # 0 pass · 1 assertion fail · 2 panic · 3 timeout
```
**Script format (RON)** — deterministic bot actions + assertions:
```ron
Script(
  seed: "harness-001", creative: false,
  steps: [
    At(0,   Teleport((0.0, 70.0, 0.0))),
    At(1,   Give("us:wood_pickaxe", 1)),
    At(2,   SetMode(Terrain)),
    At(10,  Look(yaw: 0.0, pitch: -30.0)),
    At(20,  Dig(target: LookRay, bites: 12)),
    At(120, AssertThat(ChunkEdited((0, 2, 0)))),
    At(121, AssertThat(InventoryHas("us:dirt", min: 1))),
    At(130, SetMode(Grid)), At(131, SelectHotbar(3)),
    At(140, Place(target: LookRay, count: 4)),
    At(200, Screenshot("out/grid_mode.png")),
    At(210, AssertThat(NoWarnings)),
  ],
)
```
**Bots** for soak: `random_walker`, `tunneler`, `builder`, `fighter` (attacks nearest enemy), `explorer` (BFS across map).

### 22.4 Debug tooling (dev builds; **never** require a human to inspect visually)
- **F3 overlay** (+ Shift-F3 variants): FPS/frame breakdown, chunk/mesh/light/nav queues, memory, net.
- **Debug draw toggles** (console `/debug <flag>`): chunk borders, SDF wireframe, normals, block grid, light levels (heatmap), nav graph/paths, colliders, hurtboxes, spawn candidates, AI perception cones.
- **Console commands**: `/give <item> [n]`, `/tp x y z`, `/time set <t>`, `/weather <w>`, `/spawn <enemy> [n]`, `/seed`, `/mode creative|survival`, `/fly`, `/edit_mode terrain|grid`, `/regen_chunk x y z`, `/dump_chunk x y z`, `/event <id>`, `/boss <id>`, `/phase <n>`, `/god`, `/nav_rebuild`, `/perf <scene>`.
- **Replay recorder**: records inputs + seed + op log; `--replay <file>` reproduces bugs deterministically (DEV-004).

| ID | Feature | Pri | M |
|---|---|---|---|
| DEV-001 | In-game console + command registry | P0 | M2 |
| DEV-002 | Debug overlays & draw toggles | P0 | M2 |
| DEV-003 | Headless harness + script runner + screenshots | P0 | M0 (skeleton) → M10 (full) |
| DEV-004 | Replay recorder/player | P1 | M9 |
| DEV-005 | Bench scenes + CSV + CI thresholds | P0 | M2 |
| CRE-001 | **Creative mode** (flight, infinite items, no damage, time control) | P1 | M5 |
| CRE-002 | Creative item browser/spawn menu + search | P1 | M5 |

### 22.5 CI pipeline (GitHub Actions or equivalent)
1. `fmt` + `clippy -D warnings` + `cargo deny` (licenses/advisories) + `cargo test --workspace`.
2. Build **Windows + Linux** release artifacts (cache with `sccache`).
3. Run **content validator** and **save-compat fixtures**.
4. **Headless smoke** (`--headless --script scripts/smoke.ron`) using **software Vulkan** (Mesa lavapipe) on Linux runner.
5. Nightly: property tests (high case count), 1000-seed worldgen fuzz, benches + threshold compare, visual regression, 30-min soak bot, MP soak (from M13).
6. Artifacts: screenshots, CSVs, crash logs, `world dumps` for failures.

### 22.6 Definition of Done (per feature)
- Acceptance criteria in the feature table met **and** covered by an automated test (unit/property/integration/harness).
- Data-driven where applicable; **validator passes**.
- Debug overlay or console command exists to inspect the feature’s state.
- Perf within budget (bench updated if feature touches hot paths).
- Server/client split respected (works in SP-local-server and dedicated MP unless explicitly marked client-only).
- Localization keys used; no hard-coded strings.
- Docs updated (`CHANGELOG.md`, ADR if needed).

---

## 23. Decisions, Assumptions & Open Questions (owner review required)

| ID | Decision / question | Recommended default | Options / notes | Needed by |
|---|---|---|---|---|
| **D-01** | **Low-poly look**: terrain lattice/cell size and facet strategy | **1 m cell** + flat shading; choose A/B/C after M1 spike | A) baseline 1 m; B) simplified render mesh; C) quantized verts. Block size must equal edit lattice (1 m) | M1 |
| **D-02** | Engine/language | **Rust + Bevy** (§3) | C++ if flip conditions (§3.4) | M0 |
| **D-03** | Edit-mode semantics (A-1) | Mutually exclusive `{Terrain, Grid}` | Independent toggles | M4 |
| **D-04** | Networking stack | Spike `bevy_replicon` vs `lightyear` vs custom `quinn` in M0–M3; choose by **edit-op replication fit + Bevy-version support** | Must support dedicated server + prediction | M3 |
| **D-05** | Starting health/mana & caps (A-3) | 5 hearts→15/20; 2 stars→10 | Mockup shows 9 hearts | M8 |
| **D-06** | First-person only vs optional third-person | FP default; 3rd-person P2 (RND-016) | | M14 |
| **D-07** | World size/height | 1024²×512 Small as default | | M6 |
| **D-08** | Crisp cube cuts in terrain (TER-016) | Accept rounded cuts for 1.0 | Dual contouring / triangle clipping | M14 |
| **D-09** | Lore, names, NPC identities, boss names | **Owner supplies**; agent uses placeholders | | M12 |
| **D-10** | Art production plan | **Placeholder-first** (§25.4); final art by owner/artist | | M1 |
| **D-11** | Audio production plan | Placeholder CC0/generated; final music/SFX commissioned | | M7 |
| **D-12** | Business model | Premium single purchase (assumed) | | M15 |
| **D-13** | Minimum supported GPU API | Vulkan 1.2 / DX12; no GL fallback | | M0 |
| **D-14** | Permadeath behavior | Spectator + character lock | Delete char | M9 |
| **D-15** | Do we allow terrain under blocks to be removed leaving floating blocks? | Yes (1.0) | Support rule P3 | M4 |
| **D-16** | Torch/light spawn suppression in caves | Yes (cave spawns need low light) | Pure darkness-independent | M8 |
| **D-17** | Multiplayer player cap | 8 | 4 or 16 | M13 |

Agents: log new questions in `docs/OPEN_QUESTIONS.md` using this table format, choose the recommended default, and **continue**.

---

## 24. 1.0 Release Checklist

**Gameplay**
- [ ] All P0 + P1 features accepted; P2 triaged (shipped or cut with notes)
- [ ] 5 bosses beatable in SP and MP; progression path completable start→end without softlocks (bot + human test)
- [ ] All strata/biomes/structures generate valid in 1,000-seed fuzz and 20 hand-checked seeds
- [ ] Balance pass for all difficulty modes; economy sanity pass; no infinite item/currency exploits

**Stability**
- [ ] 0 known crashers; crash-free session rate target ≥ 99.5 % in beta
- [ ] Save/load fuzz and corruption tests pass; all older fixtures migrate
- [ ] 4-hour soak (SP) and 2-hour 4-client soak (MP) with no leaks (RSS growth < 5 %)
- [ ] Desync/duplication tests pass (items, chunks)

**Performance**
- [ ] Min-spec hits 60 FPS Medium on benches (`cave_dense`, `surface_forest`, `swarm_30`, `dig_stress`)
- [ ] §19.3 latency/streaming targets met; no frame spike > 20 ms in a 30-min route (excluding loading screens)

**UX/Accessibility/Localization**
- [ ] All ACC items verified; full gamepad playthrough; UI at 720p–4K, 16:9/16:10/21:9
- [ ] Tutorial/hints validated by 5 first-time testers
- [ ] Localization complete for launch languages; pseudo-loc clean

**Content/Legal**
- [ ] All assets licensed/owned; `cargo deny` clean; third-party notices file
- [ ] No Terraria/Minecraft/other-IP names/assets/text; trademark scrub of names
- [ ] Age rating questionnaire; privacy policy; EULA; opt-in telemetry verified

**Platform/Store**
- [ ] Windows + Linux builds signed/packaged; Steam depot, achievements, cloud saves; Deck test (if targeted)
- [ ] Store page assets, trailer (use harness camera paths), patch notes, support docs, modding guide (data-only)

---

## 25. Appendices

### 25.1 Glossary
| Term | Meaning |
|---|---|
| **Strata (stratum)** | Depth-based world layer with its own materials, enemies, hazards, music |
| **SDF** | Signed distance field; negative inside solid, positive in air |
| **Cell / CELL** | 1 m lattice unit shared by terrain samples and blocks |
| **Chunk** | 32³-cell storage/meshing unit |
| **Surface Nets** | Meshing algorithm producing one vertex per surface-crossing cell |
| **Edit Op** | Deterministic, serializable world mutation (`EditOp`) |
| **Overlay Model** | Terrain and blocks interpenetrate; independent layers (§5.7) |
| **Grid Mode / Terrain Mode** | Precision-building vs organic-sculpting edit modes (§6) |
| **Bite** | One discrete dig brush application |
| **Yield accumulator** | Fractional drop tracker per player/material |
| **Waystone** | Fast-travel node |
| **Block entity** | Block with extra state (chest, door, sign) |
| **Viewmodel** | First-person held-item/hand render |
| **WORLDGEN_VERSION** | Integer bumped when generator output changes |
| **Interest management** | Sending only nearby world state to each client |
| **ADR** | Architecture Decision Record in `docs/DECISIONS.md` |

### 25.2 Content scope targets for 1.0 *(tunable; planning numbers)*
| Category | Target |
|---|---|
| Terrain materials | ~25 |
| Block types (incl. shapes as variants) | ~80 |
| Items total | ~350 (tools 30, weapons 45, armor 6 sets/18 pcs + vanity P2, accessories 25–30, consumables 40, materials 60, placeables/furniture 60+) |
| Recipes | ~300 |
| Enemies | ~30 + 3 mini-bosses + 5 bosses (+ 4 optional P2) |
| Structures/POIs | 14 kinds (≥ 3 variants each) |
| NPCs | 6 (+2 P2) |
| Quests | 20–30 |
| Events | 6 |
| Achievements | ~60 |
| Music | ~12 tracks + stingers |
| SFX | ~400 |
| Languages | 1 at launch (P1) → 8 (P2) |

### 25.3 Naming & file conventions
- IDs: `namespace:snake_case` (`us:copper_pickaxe`); textures/models/audio file stems `snake_case`.
- Data files: one RON file per logical group; each top-level entry has `id`. Cross-refs by id only.
- Rust: `snake_case` modules, `CamelCase` types; feature IDs in comments; no `unwrap()` in non-test code outside startup invariants; use `thiserror`/`anyhow` appropriately; errors surface in logs with context.
- Git: trunk-based, small PRs, conventional prefix = feature ID.

### 25.4 Placeholder-first asset plan (agents can start now)
Mockups are concept renders; **final art/audio must be produced**. Until then:
- **Textures**: script-generated 16×16 pixel tiles (flat palette + noise dither) per material; clearly named `placeholder_*`. Atlases built by `bake-atlas`.
- **Models**: primitive meshes (cubes/capsules) with per-enemy color; glTF stubs w/ correct scale and animation names (`idle, walk, attack, hit, death`).
- **Audio**: generated/CC0 beeps/thuds; mixer wired to final categories.
- **UI**: simple pixel frames; icons as placeholder glyphs.
- **Swap path**: replacing a placeholder with final art MUST be a **data/asset-only change** (same ids, same texel density, same rig/animation names).
- Maintain `docs/ASSET_REQUESTS.md` listing every placeholder with desired dimensions/format so the owner/artist can fill them in.

### 25.5 Risk register
| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R-01 | Terrain/block seams, z-fighting, or visual mismatch with mockups | High | High | Overlay Model (§5.7), look-dev spike, bias/epsilon, early screenshots at M1/M4 |
| R-02 | Bevy API churn breaks code | High | Medium | Pin version, thin Bevy layer, upgrade only between milestones, rely on core crates |
| R-03 | Art/audio volume exceeds capacity | High | High | Placeholder-first, strict content targets §25.2, data-driven swapping, cut P2 content first |
| R-04 | Scope creep | High | High | Binding priorities; “P2 may be cut” rule; milestone exit gates |
| R-05 | Netcode complexity with edit-heavy world | Medium | High | Architecture-first (NET-001), op-based edits, sim tests early (NET-010), spike D-04 |
| R-06 | Underground rendering perf (many chunks, volumetrics) | Medium | High | Cave occlusion culling, LOD, quality presets, benches in CI |
| R-07 | Player stuck/tunneling after edits | Medium | High | SDF capsule controller, depenetration tests, fuzz bots |
| R-08 | Nav invalidation churn from constant edits | Medium | Medium | Chunk-local async rebuild, revalidation-on-use, fallback steering |
| R-09 | Worldgen non-determinism across platforms | Medium | High | In-house noise, `libm`, integer hashing, golden tests on Win+Linux CI |
| R-10 | Save corruption / migration bugs | Low | Critical | Atomic writes, CRCs, fixtures, backups |
| R-11 | Light propagation hitches on big edits | Medium | Medium | Incremental BFS, per-frame budget, async |
| R-12 | AI-agent drift (inconsistent architecture) | Medium | Medium | ADRs, crate boundaries enforced by `cargo deny`/lints, this spec as source of truth |

### 25.6 Agent task template (copy for every ticket)
```
TASK: <FEATURE-ID> — <title>
SPEC REFS: §<x.y>, §<a.b>
GOAL: <one sentence, user-visible outcome>
INPUTS: <crates/files/data to touch; new data schemas>
STEPS: 1) … 2) … 3) …
ACCEPTANCE: <copy from feature table> + <automated tests to add>
HARNESS CHECK: <script/bench/screenshot command proving it>
OUT OF SCOPE: <explicitly excluded adjacent features>
PERF/BUDGET: <relevant §19 budgets>
OPEN QUESTIONS: <log in docs/OPEN_QUESTIONS.md if any>
```

### 25.7 First 12 tasks (so an agent can start immediately)
1. **M0** Create workspace per §4.2; pin toolchain + Bevy; CI with fmt/clippy/test/build (PLT-001).
2. **M0** Add `us_core` coords/constants (§4.5) with `div_euclid` tests; deterministic RNG + noise module + golden tests.
3. **M0** Headless harness skeleton: parse CLI (§22.3), run N ticks, exit codes, JSON logs, screenshot stub (DEV-003).
4. **M0** Content system skeleton: RON loaders, id registry, validator CLI with one sample item/block/material.
5. **M1** `us_voxel`: `Field<T>`, `TerrainChunk`, SDF quantization, uniform collapse, tests (TER-001).
6. **M1** `us_mesh`: padded snapshot + surface nets + flat shading + material per tri; property test for watertightness (TER-002).
7. **M1** `us_render`: custom terrain material (triplanar tex-array, nearest/mip) + fly camera + test terrain (RND-001, MOV-011).
8. **M1** Look-dev spike A/B/C with 6 fixed-camera screenshots; write ADR for D-01 (TER-003).
9. **M2** `EditOp` + `apply` + op log + dirty tracking + replay test (TER-009).
10. **M2** Async remesh scheduler with priority, no-flicker swap, bench scene `dig_stress` (TER-007).
11. **M2** Dig brush with bites, reach, hardness/tier, yield accumulator, drops + magnet (TER-004/008/019, QOL-001).
12. **M2** Console + debug overlays (DEV-001/002) so every later feature is inspectable.

### 25.8 Document maintenance
This file is the **source of truth**. When implementation reality diverges, **update this spec in the same PR** (and add an ADR). Version history lives in `CHANGELOG.md` under a “Spec” heading.

---
*End of specification — v0.1*
