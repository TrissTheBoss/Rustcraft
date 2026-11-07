# Voxel Engine

Voxel Engine is a clean-room Rust project for building an original voxel game engine. It is not a Minecraft launcher, client, mod, or distribution of Minecraft.

## Scope

The roadmap covers original implementations of rendering, world/chunk storage and streaming, meshing, lighting, entities, collision, deterministic simulation, networking, resource formats, and developer tooling.

The intended final product, including detailed menus and settings, gameplay systems, release gates, and a versioned Java Edition parity program, is specified in [prd.md](prd.md). The PRD describes future requirements; it is not a statement that they are implemented.

The current `0.8.8` build contains the world foundation, deterministic terrain generation, a Java Edition 26.3-targeted movement controller wired into the Windows player loop, fixed-step simulation with voxel collision queries and replay snapshots, and an optimized dependency-free CPU rendering pipeline. Movement advances at 20 Hz, uses explicit 16-block chunk/grid coordinates, supports one-block jump clearance, and caps airborne horizontal speed to prevent momentum accumulation.

The Windows scene currently opens a bounded generated region in Creative prototype mode. The native **Worlds** menu can create a world with an editable name, optional decimal/hex seed, four stored modes, four stored difficulties, and an optional difficulty lock; reopen, edit options, copy, delete, and restore listed saves; **File** can save dirty changes or quit. Existing worlds can be renamed and their seed/mode/difficulty metadata edited; changing a seed does not regenerate existing terrain. Difficulty is saved metadata only and does not yet affect gameplay. Adventure is preserved across save/load, uses ground movement without Creative flight, and rejects all block edits until Adventure permission/item rules are implemented; it is not full Adventure behavior. Spectator and Survival systems remain incomplete prototypes. Delete and restore require confirmation; delete cannot target the active world, and restore preserves its current in-memory state as a backup. The transactional inventory core now persists with player saves in format v3, including hotbar selection; keys 1-9 select a persisted slot, while prototype block selection and block edits still use a separate temporary path. A prototype block-loot and dropped-item simulation models pickup delay, gravity, partial inventory pickup, and expiry, but it is not yet connected to block-breaking input, world saves, or rendering. An in-game inventory screen remains missing. Save formats v1 and v2 migrate with an empty inventory. Terrain options, backup preview/restore-as-copy, pause/settings screens, and the finished styled interface are still missing. Left click breaks the targeted block; right click places the selected prototype block; keys 1-3 select soil, surface, and rock. Edits use loaded chunks only and are applied on the simulation tick. This is not the finished inventory or survival-mining flow. Storage and option migration are tested, but interactive menu/world switching and the GPU hardware path still need runtime verification.

## Compatibility boundary

This project contains no Minecraft source, decompiled code, proprietary assets, authentication bypasses, or copied protocol implementation. It does not currently connect to ordinary Minecraft servers and makes no claim of vanilla parity.

“100% vanilla parity” is an acceptance target requiring exhaustive independent testing and legal review; it cannot be promised in advance. Any future compatibility work must use public specifications, independently authored tests, and user-owned environments.

## Build

```text
cargo fmt --check
cargo test
cargo run --release
```

For repeatable local testing, run `.uild-release.ps1`; it refreshes `target/release/voxel-engine.exe`. The executable is generated output and remains intentionally excluded from Git.

On Windows, the release command opens the interactive prototype, which stores worlds under `%LOCALAPPDATA%\VoxelEngine\worlds`, autosaves dirty state every 30 seconds, saves on normal close, and retains up to five backups. The storage layer is tested; the integrated window lifecycle and native world menu have compile coverage but still need runtime verification. On other platforms the command runs a headless CPU-rendered demo. Logs and any permitted local texture cache use their documented local paths.

For a local, user-owned Java client JAR, run `cargo run --release -- --extract-assets [path-to-client.jar]`. If no path is supplied, the utility searches the platform's default `.minecraft` location, including the Linux Flatpak location, and extracts only block textures, block `.png.mcmeta` files, and item textures into the ignored `assets/extracted/` directory. The source JAR is read-only and no network access occurs during extraction.

Generated builds, local caches, game data, credentials, and external source checkouts are excluded from Git.

## Hermes project agent

CI runs formatting, tests, and a release build through GitHub Actions.

## Detailed roadmap

### v0.2 — Headless world core

- Define stable world and chunk coordinates.
- Add a world manager with lazy chunk creation and unload support.
- Add block registry boundaries and safe block mutation APIs.
- Implement deterministic, dependency-free chunk persistence.
- Add round-trip, bounds, and deterministic update tests.

### v0.3 — Deterministic simulation

- Add a fixed-timestep clock.
- Add entity storage, lifecycle, transforms, and collision primitives.
- Add gravity and movement integration.
- Add deterministic replay fixtures and simulation regression tests.

Simulation implementation status: `src/simulation.rs` provides a fixed-step clock, stable entity lifecycle storage, gravity, voxel occupancy collision queries, ground-plane collision, snapshots, and regression tests.

### v0.4 — Rendering foundation

- Select and document the window/GPU stack, initially evaluating `wgpu`.
- Add input, camera, frame scheduling, and renderer abstraction layers.
- Add chunk meshing, face culling, texture/resource loading, and basic lighting.
- Add frame-time and memory profiling before optimization decisions.

Rendering implementation status: the CPU pipeline is implemented in `src/render.rs`, and `src/window.rs` provides a decorated native Windows window sized to the primary monitor with a 20 Hz simulation update loop. Mouse deltas are measured from the window center, so recentering no longer cancels camera movement. The cursor is hidden while captured; Escape releases it and clicking captures it again. GPU presentation and a complete game loop remain pending.

### v0.5 — Developer tooling

Terrain implementation status: `src/terrain.rs` generates deterministic seeded chunks with varied surface, soil, and rock layers. These are original engine blocks and procedural colors; Minecraft-owned textures and assets are intentionally not included.

Large-world rendering status: the prototype generates a bounded chunk region, meshes each chunk with neighboring occupancy checks, and translates meshes into world space. Runtime chunk streaming and GPU batching remain the next scale steps.

Grid contract: one block is 1×1×1 world unit; chunks are 16×16×16 blocks; the player is 0.6 blocks wide and 1.8 blocks tall with a 1.62-block eye height and 0.6-block step height. Frustum tests use all eight chunk-bound corners and conservatively retain near-plane intersections.

The window title reports the player’s world-space block coordinates and horizontal speed in blocks/second during testing.

Performance status: greedy meshing merges coplanar faces with matching block IDs and scales UVs for repeated textures. Mesh bounds are tested against the camera frustum before rasterization. Terrain and meshes are cached instead of rebuilt every frame, and the CPU render buffer is capped at 960x600 before scaling to the native window. The CPU renderer remains a prototype and is not yet a substitute for GPU batching.

Movement status: `src/movement.rs` is wired to the Windows player loop. W/A/S/D drive the body, Shift sprints, Space jumps, Ctrl sneaks/descends, collision queries use generated chunks, and the camera follows the player eye. The fixed cadence and single-block clearance are corrected; exact parity remains an evidence-driven compatibility target because public references document headline values but not every edge case, effect, block, or mode interaction.

- Add a world inspector and chunk visualizer.
- Add resource validation and format conversion tools.
- Add replay capture, debug overlays, crash reports, and benchmark commands.

### v0.6 — Original multiplayer

- Specify an original client/server protocol.
- Add authoritative server simulation and client prediction boundaries.
- Add authentication and transport security for the new engine only.
- Build compatibility tests against the engine's own server.

### v1.0 — Release gate

- Cross-platform builds and QA.
- Performance benchmarks with published hardware/configuration.
- Security and persistence review.
- Documented compatibility evidence; no parity claim without exhaustive testing.
