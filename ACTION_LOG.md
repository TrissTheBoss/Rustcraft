# Action log

This log records material repository actions, results, decisions, and next steps. Each milestone uses a focused commit, passes relevant checks, and receives a release tag. Generated files and local user data stay out of Git.

## 2026-11-06

- Re-scoped the repository from Rustcraft launcher/Fabric trigger to a clean-room Rust voxel engine at the user's direction. Result: launcher-specific runtime is no longer the active product. Next: build the engine foundation in incremental modules.
- Removed tracked launcher configuration, startup script, Fabric trigger project, and obsolete Cargo lockfile. Result: the active tree no longer presents a Minecraft launcher or mod as the product. Next: add original engine boundaries and documentation.
- Added original world/chunk storage, entity primitives, protocol-neutral session state, resource manifest types, and a minimal executable. Next: add deterministic simulation tests, renderer selection, and tooling interfaces.
- Documented the compatibility boundary: no Minecraft code/assets are included, and ordinary-server compatibility plus 100% parity are future acceptance targets rather than current claims.
- Expanded the roadmap into staged milestones from headless world core through simulation, rendering, tooling, original multiplayer, and release gates. Began `v0.2` implementation with world/chunk coordinates, a world manager, and dependency-free persistence. Next: finish the world core and its round-trip tests.
- Documented this conversation's durable project context in `CONVERSATION_MEMORY.md` and added `AGENTS.md` with clean-room, honesty, verification, documentation, handoff, and release rules. Result: future agents have repository-local instructions, while the file explicitly states that it cannot override higher-priority system or user instructions. Next: keep these files synchronized as the engine evolves.

## Documentation procedure

1. Record the action and result here.
2. Record material architectural choices in `DECISIONS.md`.
3. Update `README.md` when user-facing scope or commands change.
4. Keep commits focused and tag usable milestones.
5. Never commit credentials, proprietary assets/source, generated builds, or copied external repositories.

## Rough roadmap

- `0.1`: core data model and tests.
- `0.2`: deterministic world simulation and persistence.
- `0.3`: renderer abstraction and first original renderer.
- `0.4`: entity/collision systems and developer tooling.
- `0.5`: original multiplayer server/client protocol.
- `1.0`: only after cross-platform QA, security review, performance profiling, and documented compatibility evidence.

- Optimized active chunk persistence to stream the contiguous block array directly rather than perform coordinate indexing and bounds validation for each block. Result: save/load exercise the optimized path with the same serialized block order and format. Verification pending: formatting, tests, and release build. Next: profile realistic runtime workloads once a renderer exists.
- Started the rendering milestone with a dependency-free CPU pipeline: camera and input state, projection/view math, boundary-aware visible-face meshing, P6 texture loading and sampling, material registry, directional lighting, depth-tested triangle rendering, frame pacing, and bounded frame-time/memory-size reporting. Result: the offline renderer is executable as a library path and has focused unit coverage. Attempted to add `wgpu` and `winit`; Cargo could not reach crates.io, and no graphics crates are cached. Next: integrate a windowed GPU backend and verify interactive presentation when dependency access is available; rendering is not yet complete as a product milestone.
- Turned the executable into an end-to-end headless prototype demo. Result: `cargo run --release` now generates an original flat chunk, culls internal faces, rasterizes a 320x200 frame with the CPU renderer, and reports mesh and visibility metrics without writing local world data. Verification: formatting, tests, and release build run after this change. Next: integrate a platform window/GPU adapter when dependency access is available.
- Started v0.3 deterministic simulation. Added a fixed-step accumulator, stable entity lifecycle storage, gravity integration, and ground-plane collision with focused regression tests. Next: connect simulation bodies to voxel collision queries and add deterministic replay fixtures.
- Completed the next v0.3 simulation increment. Added voxel occupancy collision queries, stable simulation snapshots, and deterministic replay regression coverage. Verification: formatting and tests run after this change; release build and prototype run remain before publication. Next: begin v0.4 platform rendering integration when graphics dependencies are available.
- Began v0.4 as a playable-surface milestone. Added a native Windows user32/gdi32 window adapter that presents the CPU-rendered voxel scene in a launchable executable, while retaining the headless fallback on other platforms. Verification: formatting, 14 tests, release build, and diff checks passed. Next: add continuous frame updates and keyboard/mouse camera input.
- Added the first interactive game-loop behavior. The Windows window now updates at 30 Hz and supports W/A/S/D movement plus Space/Ctrl vertical camera movement. Verification: formatting, 14 tests, release build, and diff checks passed. Next: add mouse look and a controllable player body using the simulation collision system.
- Added first-person mouse look to the prototype window. Mouse deltas now update yaw/pitch with vertical look clamping, while the existing movement loop continues to refresh the rendered scene. Verification: formatting, 14 tests, release build, and diff checks passed. Next: connect a player body and block interaction to the simulation/world systems.
- Fixed the Windows prototype presentation usability. The scene now renders at 960x600 instead of stretching a 320x200 buffer, and the cursor is captured to the window during first-person play. Escape releases capture; clicking the window captures it again. Verification: formatting, 14 tests, release build, and diff checks passed. Next: connect a player body and block interaction to the simulation/world systems.
- Changed the Windows prototype to borderless fullscreen using the primary monitor's native resolution. The render buffer now matches that resolution, and the hidden captured cursor is recentered internally so it no longer appears visibly stuck at the center. Escape still releases capture. Verification: formatting, 14 tests, release build, and diff checks passed. Next: add a configurable window/fullscreen mode once the player loop is more complete.
- Fixed camera input cancellation by measuring mouse movement from the window center before recentering. Restored decorated windowed mode while retaining the primary monitor's width and height for the initial window size. Verification: formatting, 14 tests, release build, and diff checks passed. Next: validate player movement and add configurable resize/fullscreen handling.
