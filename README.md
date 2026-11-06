# Voxel Engine

Voxel Engine is a clean-room Rust project for building an original voxel game engine. It is not a Minecraft launcher, client, mod, or distribution of Minecraft.

## Scope

The roadmap covers original implementations of rendering, world/chunk storage and streaming, meshing, lighting, entities, collision, deterministic simulation, networking, resource formats, and developer tooling.

The current `0.5.0` build contains the world foundation, deterministic terrain generation, fixed-step simulation with voxel collision queries and replay snapshots, and a dependency-free CPU rendering pipeline. On Windows, running the binary opens a native prototype window displaying generated layered terrain; non-Windows builds retain the headless metrics demo. It does not yet provide original texture packs, a GPU backend, or a complete game loop.

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

Rendering implementation status: the CPU pipeline is implemented in `src/render.rs`, and `src/window.rs` provides a decorated native Windows window sized to the primary monitor with a 30 Hz update loop. Mouse deltas are measured from the window center, so recentering no longer cancels camera movement. The cursor is hidden while captured; Escape releases it and clicking captures it again. W/A/S/D move the camera, Space moves up, and Ctrl moves down. GPU presentation and a complete game loop remain pending.

### v0.5 — Developer tooling

Terrain implementation status: `src/terrain.rs` generates deterministic seeded chunks with varied surface, soil, and rock layers. These are original engine blocks and procedural colors; Minecraft-owned textures and assets are intentionally not included.

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
