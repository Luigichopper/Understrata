# Dependencies Audit

| Crate | Purpose | License | Maintenance | Bevy Compatibility | Notes |
|---|---|---|---|---|---|
| `bevy` (0.19.1) | Game engine & rendering shell | MIT OR Apache-2.0 | Active | 0.19.1 | Pinned exactly |
| `glam` (0.34) | 3D math library (Vec3, IVec3) | MIT OR Apache-2.0 | Active | Compatible | Used in pure crates |
| `serde` (1.0) | Serialization framework | MIT OR Apache-2.0 | Active | Compatible | Generic |
| `ron` (0.12) | Rusty Object Notation for content files | MIT OR Apache-2.0 | Active | Compatible | Human-readable data |
| `libm` (0.2) | Pure Rust math functions for cross-platform determinism | MIT OR Apache-2.0 | Active | Compatible | Ensures float determinism |
| `clap` (4.5) | Command-line argument parser | MIT OR Apache-2.0 | Active | Compatible | Used by understrata & us_tools |
| `tracing` (0.1) | Structured logging & diagnostics | MIT | Active | Compatible | Standard logging |
| `tracing-subscriber` (0.3) | Logging formatters (JSON, env filter) | MIT | Active | Compatible | JSON output for test harness |
| `thiserror` (2.0) | Structured error definitions | MIT OR Apache-2.0 | Active | Compatible | Standard Rust errors |
| `anyhow` (1.0) | Error handling for application entrypoints | MIT OR Apache-2.0 | Active | Compatible | Binary error handling |
