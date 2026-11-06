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
