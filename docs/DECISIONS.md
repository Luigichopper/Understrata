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
