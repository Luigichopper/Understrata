# Architecture Decision Records (ADRs)

## ADR-0001: Technology Stack Selection — Rust + Bevy
- **Status**: Accepted
- **Context**: Need a fast, reliable, cross-platform engine with strong typing, data race safety for multithreaded chunk streaming, and rapid AI-agent feedback loops.
- **Decision**: Use Rust with Bevy 0.19.1 as the outer presentation shell, while maintaining an engine-agnostic core across pure crates (`us_core` through `us_net`).
- **Consequences**:
  - Compiler enforces thread safety across parallel meshing and generation.
  - Pure crates can be tested and verified headlessly without GPU or window dependencies.
  - Bevy API updates are isolated to presentation crates.

## ADR-0002: Headless Test Harness & CI Contract
- **Status**: Accepted
- **Context**: Automated agent verification requires running the simulation, headless testing, scripted bots, and frame counting without requiring an interactive window.
- **Decision**: `understrata` binary supports `--headless`, `--frames N`, `--ticks N`, `--script`, and outputs structured JSON logs.
- **Consequences**: Every feature is testable in CI and automated runs.

## ADR-0003: Low-Poly Faceted Surface Nets Mesher (D-01)
- **Status**: Accepted (Baseline A)
- **Context**: Milestone 1 requires rendering diggable smooth SDF terrain with a readable low-poly faceted look (MOC-002, TER-002).
- **Decision**: Use a 1 m lattice with naive Surface Nets on a 34³ padded snapshot (`PaddedChunkArray`), outputting unindexed triangles with flat face normals. Surface skins convert up-facing (`normal.y > 0.7`) dirt triangles to grass (TER-011).
- **Consequences**:
  - Watertight meshing across chunk boundaries without gaps or T-junctions.
  - Zero heap allocations for uniform chunks via `Field::Uniform`.
  - Crisp, faceted visual appearance matching the concept art silhouettes.

## ADR-0004: Kinematic Capsule Controller vs Continuous SDF (MOV-001, NET-001)
- **Status**: Accepted
- **Context**: Milestone 3 requires player movement physics without ghost collisions, mesh rebuild latency desyncs, or tunneling through terrain. Single-player mutations must follow authoritative local-server architecture.
- **Decision**:
  - Implement a pure kinematic capsule collision controller in `us_sim` operating directly against the continuous signed distance field and surface gradients (`gradient_at_world`), completely bypassing triangle mesh colliders.
  - Implement move-and-slide projection, slope sliding only on very steep hills (`normal.y < cos(65°)`), step-up up to 0.6 m, and post-edit depenetration pushing out along the gradient (no teleport > 1.5 m).
  - Single-player gameplay routes all player input and world edit intents through `LocalServer`, which validates actions, steps character physics, and emits authoritative state deltas (`PlayerStateDelta`, `EditOpDelta`).
- **Consequences**:
  - Instantaneous terrain collision response with zero mesh-collider generation latency after edits.
  - Pure deterministic simulation testable headlessly via automated fuzz runners.
  - Single-player and multiplayer use the exact same server-authoritative delta architecture.
