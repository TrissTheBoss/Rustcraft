# Project conversation memory

This file is a durable project summary, not hidden model memory. It exists so future contributors and AI agents can recover context from the repository.

## Product transition

The project began as Rustcraft, a user-owned Minecraft command launcher with a local Fabric trigger. The launcher and trigger implementation were later intentionally scrapped. The active product is now a separate clean-room Rust voxel engine.

## Decisions already made

- The engine must be written in Rust.
- No Minecraft proprietary code or assets may be included.
- No authentication or ownership bypasses are allowed.
- Existing Minecraft-server compatibility and 100% vanilla parity are not currently implemented or promised.
- Git history is retained for auditability, while the active tree contains only the new engine foundation.
- Roadmap order: headless world core, deterministic simulation, rendering, tooling, original multiplayer, then release validation.

## Current implementation

`voxel-v0.2.0` contains chunk/block storage, chunk coordinates, a lazy world manager, deterministic dependency-free persistence, entity primitives, resource manifest primitives, and a protocol-neutral networking boundary.

The current verification state is formatting clean, 2 unit tests passing, and release build passing.

## Next step

Implement deterministic simulation: fixed-timestep updates, entity storage, movement, gravity, and collision primitives, with regression tests and documentation updates.

## Documentation rule

All future agents must update `ACTION_LOG.md` for actions/results/next steps, `DECISIONS.md` for architectural choices, `README.md` for user-facing scope, and this file when a durable project decision or milestone changes.
