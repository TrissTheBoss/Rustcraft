# Voxel Engine

Voxel Engine is a clean-room Rust project for building an original voxel game engine. It is not a Minecraft launcher, client, mod, or distribution of Minecraft.

## Scope

The roadmap covers original implementations of rendering, world/chunk storage and streaming, meshing, lighting, entities, collision, deterministic simulation, networking, resource formats, and developer tooling.

The current `0.8.2` build contains the world foundation, deterministic terrain generation, a Java Edition 26.3-targeted movement controller wired into the Windows player loop, fixed-step simulation with voxel collision queries and replay snapshots, and an optimized dependency-free CPU rendering pipeline. Movement advances at 20 Hz, uses explicit 16-block chunk/grid coordinates, supports one-block jump clearance, and has regression-tested collision behavior.

## Compatibility boundary

This project contains no Minecraft source, decompiled code, proprietary assets, authentication bypasses, or copied protocol implementation. It does not currently connect to ordinary Minecraft servers and makes no claim of vanilla parity.

“100% vanilla parity” is an acceptance target requiring exhaustive independent testing and legal review; it cannot be promised in advance. Any future compatibility work must use public specifications, independently authored tests, and user-owned environments.

## Build

```text
cargo fmt --check
cargo test
cargo run --release
```

The release command executes the headless prototype scene and does not write world or build data.

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
