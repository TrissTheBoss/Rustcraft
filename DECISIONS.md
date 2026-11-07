# Architecture decisions

## 2026-11-07 - Order backup retention by numeric time and sequence

Backup filenames contain a Unix timestamp and a process-wide numeric sequence. Retention parses and compares both numbers instead of sorting filenames lexicographically, which can misorder sequences once the counter reaches two or more digits and delete a fresh recovery point. A preserved user-selected backup remains pinned while the oldest unpinned snapshot is pruned.

## 2026-11-07 - Persist sparse block states in world payload v3

Chunks store only block-state overrides from their prototype defaults. Upper/lower slabs and snow-layer counts are validated, versioned, serialized, and applied to collision and ray selection; replacement resets to the new block default. Payload v1/v2 remains readable and derives known prototype defaults. The outer save envelope remains v6 because the inner world payload is separately versioned. The renderer uses prototype visual boxes and removes shared/occluded partial faces; fence and pane collision, selection, and visual shapes are separate. Reference models, connection-dependent geometry, catalogs, and complete reference behavior remain outstanding.

## 2026-11-07 - Persist prototype fence connection masks in payload v4

Fence state stores a four-bit north/east/south/west mask. Its set directions add prototype rail boxes to both collision and visual shapes; the empty mask leaves the post. V4 adds the mask in previously reserved state-code bits while retaining V1-V3 decoding rules, including rejecting those new bits under V3. Gameplay placement/breaking recomputes nearby masks using the current prototype rule (connect to fences and full cubes). Terrain streaming refreshes the shared horizontal perimeter of newly loaded chunks. Material-sensitive reference rules, reference geometry, and an in-game state editor remain unimplemented. This is a prototype update path, not parity evidence.

## 2026-11-07 - Use block-local collision geometry for movement and targeting

The block registry supplies prototype collision boxes and the runtime player collider and raycaster query those boxes instead of assuming each non-air block occupies a full cube. Ray traversal checks nearby owner cells so tall shapes can be selected above their base voxel. Collision and selection geometry are still coupled, block states are not stored in world data, and rendering/content definitions are incomplete; these prototypes are not Java Edition conformance evidence.

## 2026-11-07 - Generate terrain around the active player region

The world store distinguishes the editable/displayed seed from an immutable terrain-generation seed. Save v6 persists both plus the generator version; v1-v4 migrate the generation seed from their existing seed field, and v1-v5 migrate the generator version to the current implementation. Copies preserve the source generation seed even after the displayed seed has been edited. The scene ensures a bounded 5x5 horizontal region exists around the player's chunk and rebuilds the active render mesh set when the player crosses a chunk boundary. Generation preserves existing chunk data and uses the generation seed, so revisiting and reopening a world does not replace edits or vary generated terrain. Save payload v2 marks generated chunks as regenerable; v1 world payloads load conservatively as persistent. The renderer evicts generated chunks outside its retained region, while edited and legacy chunks remain persistent. Region-file paging for very large edited worlds remains outstanding. Current terrain remains original prototype content and has no Java Edition conformance evidence.

## 2026-11-06 — Replace launcher product with clean-room engine

Rustcraft launcher and Fabric-trigger code were removed from the active tree after the product direction changed. Git history remains intact for auditability; the active product is now Voxel Engine.

## 2026-11-06 — No copied Minecraft implementation

The engine uses original code, original assets, and independently authored specifications. Existing Minecraft code, proprietary assets, authentication systems, and copied protocol implementations are out of scope.

## 2026-11-06 — Compatibility is a gated research track

Ordinary Minecraft-server compatibility and complete vanilla parity are not current features. They require public-specification study, legal review, conformance tests, and explicit evidence before any claim.

## 2026-11-06 — Product requirements describe parity as a versioned end state

The product destination includes a detailed Java Edition parity ambition, but parity is evaluated against one pinned release and a documented observable-behavior matrix. The PRD distinguishes this end state from implemented functionality and preserves the clean-room, original-content, and local-only user-owned asset boundaries. Exact conformance baselines are chosen before that work begins.

The parity scope is the complete unmodified game runtime, including all built-in gameplay and client behavior; it does not make this project a launcher or distributor of game files. Any platform-service or account-dependent limitation must be disclosed, and it prevents an unqualified 100% claim unless supported through authorized flows.

## 2026-11-07 — Pin the first Java Edition conformance baseline

The first target is the official stable Minecraft: Java Edition 26.3 release, announced 2026-09-15. This matches the release family referenced by earlier movement experiments but does not treat those experiments as conformance evidence. The initial test cell is Windows x86-64, unmodified single-player with the integrated server, default resources/options, English (US), and keyboard/mouse. Other platforms, contexts, and account-dependent services require separate cells. Per-run launcher manifest, artifact hashes, and Java runtime must be captured from an authorized local installation before a case can be verified. The baseline record is `docs/compatibility/java-26.3.md`; selecting it proves no feature behavior.

## 2026-11-07 — Pause before resuming or saving the active world

The Escape pause screen is modal and sets the same local simulation pause gate used by other native forms. It clears held movement/mining input, releases captured mouse state, and restores the prior capture state only on Resume. Save & Quit persists dirty world/player state first; a persistence error leaves the screen open for recovery. Closing the application from the window chrome retains the existing window-close save path. This is a functional native prototype; settings, accessibility, disconnect, and reference-specific menu behavior remain incomplete.

## 2026-11-07 — Keep local settings versioned and separate from worlds

Player preferences use a bounded binary schema under the application data directory, separate from world snapshots and server/world rules. Version 3 stores field of view, mouse sensitivity, Y-axis inversion, the seven implemented movement bindings, and Hold/Toggle behavior for sprint and sneak. Versions 1 and 2 migrate to Hold behavior; v1 also receives default bindings. Values load at startup and apply to camera/input. Saves use the world's durable atomic-replacement helper. A checksum detects accidental corruption, unsupported schema versions fall back to defaults with a diagnostic, and invalid fields/groups fall back safely. The native options screen detects key conflicts and requires confirmation before sharing a binding; Escape cancels capture. Escape/inventory/hotbar keys are reserved. Focus loss clears held keys; configured toggle states persist until toggled again or settings are applied. Current bounds/defaults and input behavior are engine prototype choices and remain unverified against the pinned reference. The native options screen applies settings only after validation and successful persistence; cancel/back discards unapplied edits.

## 2026-11-06 — Foundation before renderer

The first implementation establishes testable world, entity, resource, and networking boundaries before selecting a renderer or platform abstraction.

## 2026-11-06 — Stream chunk persistence from contiguous storage

Chunk persistence reads and writes the existing contiguous block array directly, preserving the y/z/x serialized order while avoiding coordinate indexing and bounds checks for each block. This is a focused runtime optimization of the active world core and does not change the persistence format.

## 2026-11-07 — Persist provisional player survival state in save v7

Save format v7 adds bounded player health and fall-distance/arming state so the current Survival prototype can resume across save/load. Versions 1-6 remain readable and default to full health with no pending fall. The current 20-point health scale, 3-block safe distance, integer damage calculation, spawn choice, death-drop pickup timing, and respawn behavior are prototype decisions only. They require observable fixtures against the pinned Java Edition build before any conformance claim. Death is persisted through health zero; respawn is saved before leaving the death screen so a save failure cannot silently discard the dead state. The default death flow transactionally moves all held/equipped stacks into world drops; if any stack cannot be accepted, the whole transfer fails and the inventory remains intact. A loaded legacy dead save with inventory retries that migration before showing the death screen.

## 2026-11-07 — Draw an original native overlay for core player status

The Windows presentation path draws a minimal crosshair, hotbar, prototype item swatches/counts, and Survival/Adventure health pips after the scene frame. It uses the window's GDI paint pass so it can sit above either the CPU renderer or the GPU surface; when the GPU path is active, stale CPU-frame blits are skipped. This overlay establishes readable gameplay feedback, not reference styling or parity. Runtime presentation across GPU drivers, window sizes, modes, and accessibility settings still needs verification; hunger, armor, air, effects, and messages remain future HUD components.

## 2026-11-06 — Fixed-step deterministic simulation

Simulation advances through an explicit fixed-step accumulator and stores bodies in a `BTreeMap` so tick order is stable by entity ID. The first collision boundary is an original ground plane at y=0; richer voxel collision remains a later increment.

## 2026-11-07 — Queue world edits for simulation ticks

Player block actions enter a bounded input queue and mutate only loaded chunks on the 20 Hz simulation tick. Ray hits are revalidated at mutation time, placement checks collision-shape overlap with the player body, and changed meshes are rebuilt for the edited chunk and face-adjacent chunk boundaries. This provides a deterministic Creative prototype path without claiming Survival mining, inventory transactions, or complete placement rules.

## 2026-11-07 — Persist world and player snapshots as bounded atomic saves

The Windows prototype uses an application-data world directory, a versioned single-file snapshot containing world metadata, chunk data, and player state, and a checksum for accidental corruption detection. Decoding enforces file/chunk/name/player bounds and rejects duplicate chunks, truncation, trailing bytes, and unsupported versions. Saves replace through a same-directory temporary file with a durable Windows replacement operation, and preserve up to five prior snapshots. Until the world-selection UI exists, the client opens one fixed Creative prototype world; corrupt saves are logged and left untouched rather than regenerated over.

## 2026-11-07 — Use a native menu as the first world-selection bridge

The Windows prototype exposes save, create, edit options, copy, delete, open-ready-world, and backup recovery actions through a native menu so persisted worlds can be exercised before a complete custom UI exists. Corrupt, missing, and newer-format worlds are displayed as unavailable; switching saves the current dirty world first and is blocked if that save fails. Save format v2 adds difficulty and difficulty-lock metadata while v1 saves load with Normal/unlocked defaults and upgrade on their next save. Editing the seed updates metadata only; it does not regenerate existing chunks. Difficulty locking requires explicitly unlocking before changing an already locked difficulty. These controls do not yet implement difficulty or all mode rules in gameplay and do not satisfy the PRD's final menu/UI requirements.

## 2026-11-07 â€” Preserve Adventure mode and deny unsupported edits

Adventure remains a distinct stored/player mode when a world is reopened. The current player controller applies ordinary ground movement and disallows Creative flight; block edits are rejected until the engine has item permissions and block tags. This conservative behavior prevents the prototype from silently treating Adventure as Survival or granting unsupported edits; it is explicitly incomplete relative to the pinned game's Adventure rules.

## 2026-11-07 â€” Centralize inventory mutations as transactions

The inventory domain owns stack limits, item identity, player slots, equipment compatibility, cursor clicks, transfers, swaps, insertion, and consumption. UI code must call these operations rather than mutating slots directly. Exact transfers and swaps validate before changing either side; insertion may return an explicit remainder, and an inventory menu cannot discard an unreturned cursor stack. Save format v3 persists the model. A native Windows screen now exposes all main, armor, offhand, and hotbar slots and routes primary/secondary clicks through this API. Cursor drops validate capacity and ID availability, spawn into the persisted world-drop system, then clear the cursor; failed spawns preserve the cursor stack. Closing returns the cursor stack transactionally or remains open if capacity is insufficient. This is an interaction prototype, not the finished reference UI. Item components, catalog-specific validation, full reference slot semantics, crafting, item use, accessibility polish, and runtime verification remain outstanding.

## 2026-11-07 â€” Persist bounded inventory state in save format v3

Player saves now encode 36 main slots, four armor slots, one offhand slot, item ID/variant/count, and selected hotbar index. The decoder enforces a fixed slot count, bounded stack counts, single-item armor slots, and a valid hotbar index. v1 and v2 saves migrate to empty inventory; their next save writes v3. Catalog-specific item identity/equipment validation must run when content definitions are available, and world-drop transactions are still separate work.

## 2026-11-07 â€” Model block loot and dropped-item pickup independently

The first drop subsystem uses a data-driven block-to-item table, stable IDs, bounded entity count, fixed-tick gravity/settling, pickup delay, partial inventory insertion, and bounded expiry. Its starter loot and timing values are prototype-only and are not claims about any Java Edition baseline. Save format v4 now persists dropped-item IDs, stacks, transforms, pickup delay, age, and next-ID state. The Windows interaction loop and renderer do not yet use the subsystem; integration must preserve drops through switching, restart, and backup recovery before this can satisfy the PRD.

## 2026-11-07 â€” Persist dropped-item entities in save format v4

Save v4 stores bounded dropped-item state alongside the player inventory, and decodes IDs, counts, positions, velocities, age, pickup delay, and allocator state with strict bounds. v1/v2 saves migrate with empty inventory and drops; v3 preserves inventory and adds an empty drop list. Player snapshots now carry drops so backup, copy, and restore paths use the same snapshot contract. Survival prototype block breaks create mapped loot only after spawn validation succeeds; the fixed simulation tick advances items and inventory pickup, and both scene renderers draw block-colored item cubes. If entity capacity or ID allocation is exhausted, the block remains intact. Fixed-tick mining now tracks a held Survival target and cancels on release or target change. Mining profiles accept base duration, tool speed, and harvest tier; the runtime currently uses hand stats and unverified prototype timing. These values are not parity evidence. Pinned mining rules, real tool inventory, authoritative loot, merge physics, and item models remain outstanding.

## 2026-11-07 - Keep mining profiles data-driven until a parity baseline is pinned

The mining state machine and rule inputs are implemented independently of any claimed reference release. Prototype values exist only to exercise progress and cancellation in the current Survival slice. Before compatibility values are added, select the exact release/build and document observable mining, tool, and harvest behavior with independent fixtures. Existing movement references to Java 26.3 do not select this baseline.

## 2026-11-07 — Keep world management in the storage boundary

World rename, copy, and delete operations belong to `WorldStore`, so UI actions share the same validation and save semantics. Rename preserves a prior snapshot, copies receive a new world ID while retaining source seed/mode/player/world data, and deletion validates the world directory before removing it. The prototype menu confirms deletion and disallows deleting the active world. Save and directory symlinks are rejected to prevent world operations from following paths outside the store.

## 2026-11-07 — Preserve both sides of world backup recovery

Backup recovery reads and validates the selected snapshot before mutating the active save, pins that snapshot while retention prunes older files, and backs up the current in-memory world/player state before replacing the save. The prototype menu requires explicit confirmation, filters invalid backups from the choices, and reloads the active scene after a successful restore. The current workflow replaces the selected world's contents in place; preview and restore-as-copy remain requirements for the finished world-management experience.

## 2026-11-07 — Collect world creation options before generation

The prototype now collects a validated name, optional decimal/hex seed, and one of the currently exposed Survival, Creative, or Spectator modes before generating terrain. A blank seed is randomized. The simulation pauses while the native form is open, and the current dirty world must save before the new world becomes active. Difficulty, generator customization, Adventure rules, and the full cross-platform menu remain unimplemented; incomplete mode systems are not represented as finished parity.

## 2026-11-06 — Keep rendering core dependency-free until graphics dependencies are available

Implement the renderer-facing contracts and CPU rasterizer using the standard library so meshing, camera math, texture resources, lighting, and profiling can be exercised offline. The window/GPU adapter remains pending; do not represent the rendering milestone as complete until an interactive surface and GPU presentation path are integrated and verified.

## 2026-11-06 — Native Windows presentation before external graphics dependencies

The first interactive surface uses the Windows user32/gdi32 APIs directly and consumes the existing CPU renderer. This creates a testable windowed prototype without adding unavailable dependencies; it is a temporary presentation adapter, not the final cross-platform renderer.

## 2026-11-06 — Stream chunk persistence from contiguous storage

Chunk persistence reads and writes the existing contiguous block array directly, preserving the y/z/x serialized order while avoiding coordinate indexing and bounds checks for each block. This is a focused runtime optimization of the active world core and does not change the persistence format.
