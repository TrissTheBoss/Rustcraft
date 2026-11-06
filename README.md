# Voxel Engine

Voxel Engine is a clean-room Rust project for building an original voxel game engine. It is not a Minecraft launcher, client, mod, or distribution of Minecraft.

## Scope

The roadmap covers original implementations of rendering, world/chunk storage and streaming, meshing, lighting, entities, collision, deterministic simulation, networking, resource formats, and developer tooling.

The current `0.1.0` build is only a tested foundation: chunk storage, entity primitives, resource manifests, and a protocol-neutral session boundary.

## Compatibility boundary

This project contains no Minecraft source, decompiled code, proprietary assets, authentication bypasses, or copied protocol implementation. It does not currently connect to ordinary Minecraft servers and makes no claim of vanilla parity.

“100% vanilla parity” is an acceptance target requiring exhaustive independent testing and legal review; it cannot be promised in advance. Any future compatibility work must use public specifications, independently authored tests, and user-owned environments.

## Build

```text
cargo fmt --check
cargo test
cargo run --release
```

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

### v0.4 — Rendering foundation

- Select and document the window/GPU stack, initially evaluating `wgpu`.
- Add input, camera, frame scheduling, and renderer abstraction layers.
- Add chunk meshing, face culling, texture/resource loading, and basic lighting.
- Add frame-time and memory profiling before optimization decisions.

### v0.5 — Developer tooling

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
