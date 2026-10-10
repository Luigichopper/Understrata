# Open Questions & Owner Decisions

| ID | Topic | Spec Default | Status | Owner Notes |
|---|---|---|---|---|
| D-01 | Low-poly look: terrain lattice & facet strategy | 1 m cell + flat shading; evaluate A/B/C spike in M1 | Open (Spike pending M1) | |
| D-02 | Engine/language | Rust + Bevy 0.19.1 (§3) | Decided (Rust+Bevy) | |
| D-03 | Edit-mode semantics | Mutually exclusive `{Terrain, Grid}` | Accepted default | |
| D-04 | Networking stack | Spike in M3 (replicon vs lightyear vs custom) | Open for M3 | |
| D-05 | Starting health/mana & caps | 5 hearts -> 15/20; 2 stars -> 10 | Accepted default | |
| D-06 | Camera modes | First-person primary; 3rd person P2 | Accepted default | |
| D-07 | World size preset default | Small (1024x1024x512) | Accepted default | |
| D-08 | Crisp cube cuts in terrain | Surface Nets rounded cuts for 1.0; research TER-016 | Accepted default | |
| D-09 | Lore, NPC identities, boss names | Placeholders until owner supplies | In progress | |
| D-10 | Art production plan | Placeholder-first; swap via data registry | In progress | |
| D-11 | Audio production plan | Placeholder-first; mixer categories | In progress | |
| D-12 | Business model | Premium single purchase | Accepted default | |
| D-13 | Minimum supported GPU API | Vulkan 1.2 / DX12 | Accepted default | |
| D-14 | Permadeath behavior | Spectator + character lock | Accepted default | |
| D-15 | Floating block physics | Allowed in 1.0; support rule in P3 | Accepted default | |
| D-16 | Light-based cave spawn suppression | Yes, cave spawns need low light | Accepted default | |
| D-17 | Multiplayer player cap | 8 players co-op | Accepted default | |
