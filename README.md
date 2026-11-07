# Voxel Engine

Voxel Engine is a clean-room Rust project for building an original voxel game engine. It is not a Minecraft launcher, client, mod, or distribution of Minecraft.

## Scope

The roadmap covers original implementations of rendering, world/chunk storage and streaming, meshing, lighting, entities, collision, deterministic simulation, networking, resource formats, and developer tooling.

The intended final product, including detailed menus and settings, gameplay systems, release gates, and a versioned Java Edition parity program, is specified in [prd.md](prd.md). The PRD describes future requirements; it is not a statement that they are implemented.

The current `0.8.8` build contains the world foundation, deterministic terrain generation, a Java Edition 26.3-targeted movement controller wired into the Windows player loop, fixed-step simulation with voxel collision queries and replay snapshots, and an optimized dependency-free CPU rendering pipeline. Movement advances at 20 Hz, uses explicit 16-block chunk/grid coordinates, supports one-block jump clearance, and caps airborne horizontal speed to prevent momentum accumulation.

The Windows runtime loads seeded procedural terrain with an active 5×5 chunk area around the player in Creative prototype mode, then presents a modal title form. Resume World enters the active save, Singleplayer opens the existing Worlds menu, Options returns to the title, and the pause screen can save and return to title. Prototype collision definitions cover slabs, carpets, snow layers, farmland, paths, fences, and panes; player collision and ray selection use those shapes. Upper/lower slab and snow-layer count states now persist in the world payload, drive collision/ray queries, and render as simple block-local boxes. Fence states persist a north/east/south/west connection mask, and the interaction API recomputes prototype fence connections to adjacent fences and full cubes when blocks are placed or broken; Creative middle-click picks registered prototype blocks, including fences, into the hotbar; E opens an inventory screen with a case-insensitive searchable prototype-block catalog in Creative; matching blocks are selected into the hotbar. Catalog categories, cursor-stack creation, and the complete content catalog remain missing. Streaming refreshes the shared horizontal chunk perimeter; exact reference connection rules and an in-game state editor remain missing. The complete content catalog and reference-shaped models are not wired yet. Fences and panes use separate prototype collision, selection, and visual boxes; the mesher removes shared partial faces and trims partially occluded faces. The native **Worlds** menu can create a world with an editable name, optional decimal/hex seed, four stored modes, four stored difficulties, and an optional difficulty lock; reopen, edit options, copy, delete, and restore listed saves; **File** can save dirty changes or quit. Existing worlds can be renamed and their seed/mode/difficulty metadata edited; changing the editable seed does not alter the immutable terrain-generation seed. Difficulty is saved metadata only and does not yet affect gameplay. Adventure is preserved across save/load, uses ground movement without Creative flight, and rejects all block edits until Adventure permission/item rules are implemented; it is not full Adventure behavior. Spectator and Survival systems remain incomplete prototypes. Delete and restore require confirmation; delete cannot target the active world, and restore preserves its current in-memory state as a backup.

The transactional inventory core persists with player saves in format v3 and later, including hotbar selection; keys 1-9 or mouse-wheel notches select a persisted hotbar slot. Selected hotbar prototype block items drive placement; Survival consumes one item on success and Creative preserves the stack. Empty Creative slots still use the legacy 1-3 selector; the searchable catalog selects currently registered block prototypes into the hotbar. Save format v7 retains bounded dropped-item state, immutable terrain-generation inputs, prototype health, and fall-tracking state; formats v1-v6 remain readable and use defaults for newer fields. Inner world payload v4 stores sparse block-state overrides, including fence connection masks; v1-v3 payloads remain readable and use the older state schema. Survival held left-click advances fixed-tick mining progress; release or target changes cancel it, while Creative breaking remains immediate. Mining profiles accept tool speed and harvest tiers, but active gameplay uses hand mining and arbitrary prototype durations. Breaking mapped blocks atomically spawns prototype loot; drops tick with gravity, pickup delay, partial inventory pickup, and expiry, and render as small block-colored cubes through CPU and GPU paths.

Press **E** to open the native player-inventory prototype. It displays the 36 main slots, four armor slots, offhand, selected hotbar slot, and cursor stack. Primary and secondary click semantics are available through a click-mode toggle. **Drop Cursor** spawns the carried stack transactionally; in-world Q drops one item, Ctrl+Q drops the selected stack, and F swaps the selected slot with offhand. Closing returns a cursor stack to the inventory or stays open if there is insufficient space. Press **Escape** to open the pause screen; simulation stops until Resume, Save & Quit saves before closing, and Save & Return to Title saves before returning to the title screen. **Options** currently exposes field of view, mouse sensitivity, Y-axis inversion, hold/toggle choices for sprint and sneak, and rebinding for the seven implemented movement actions. Conflicts are detected and shared keys require confirmation; Escape cancels a binding capture. Inventory, hotbar, drop, offhand-swap, and pause keys remain reserved. Preferences load at startup and save separately from world data; settings format v1 migrates with default movement bindings, v1/v2 migrate with Hold behavior defaults, and invalid values safely fall back. Other settings, accessibility controls, and reference defaults remain incomplete. Prototype health, fall damage, death, respawn, and a save-and-return-to-title death screen are implemented; respawn temporarily retains inventory, and the health/fall rules are not verified against Java Edition. Crafting, item use, a styled in-game HUD, real tool inventory integration, full mining rules, and vanilla content behavior remain missing. These prototype systems are not verified vanilla rules. Save formats v1/v2 migrate with empty inventory and drops; v3 retains inventory and adds empty drops; v1-v4 initialize generation from seed metadata; v5 adds a separate generation seed; v6 adds the generator version and preserves generated-cache provenance for safe eviction; v7 adds provisional health and fall state. Terrain options, backup preview/restore-as-copy, and the finished styled interface are still missing. Right click places the selected hotbar item when it maps to a prototype block; Survival consumes the item after successful placement. Empty Creative slots retain the legacy keys 1-3 selector. Mouse-wheel selection wraps across all nine slots and persists. Edits use loaded chunks only and are applied on the simulation tick. This is not the finished inventory or survival loop. Storage and option migration are tested, but interactive menu/world switching, pause/options runtime behavior, and the GPU hardware path still need runtime verification.

## Compatibility boundary

This project contains no Minecraft source, decompiled code, proprietary assets, authentication bypasses, or copied protocol implementation. Its first conformance target is the stable Java Edition 26.3 release, recorded in [the baseline profile](docs/compatibility/java-26.3.md). It does not currently connect to ordinary Minecraft servers and makes no claim of vanilla parity; selecting a target release is not evidence of conformance.

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
