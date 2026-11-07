# Architecture decisions

## 2026-11-06 — Replace launcher product with clean-room engine

Rustcraft launcher and Fabric-trigger code were removed from the active tree after the product direction changed. Git history remains intact for auditability; the active product is now Voxel Engine.

## 2026-11-06 — No copied Minecraft implementation

The engine uses original code, original assets, and independently authored specifications. Existing Minecraft code, proprietary assets, authentication systems, and copied protocol implementations are out of scope.

## 2026-11-06 — Compatibility is a gated research track

Ordinary Minecraft-server compatibility and complete vanilla parity are not current features. They require public-specification study, legal review, conformance tests, and explicit evidence before any claim.

## 2026-11-06 — Product requirements describe parity as a versioned end state

The product destination includes a detailed Java Edition parity ambition, but parity is evaluated against one pinned release and a documented observable-behavior matrix. The PRD distinguishes this end state from implemented functionality and preserves the clean-room, original-content, and local-only user-owned asset boundaries. Exact conformance baselines are chosen before that work begins.

The parity scope is the complete unmodified game runtime, including all built-in gameplay and client behavior; it does not make this project a launcher or distributor of game files. Any platform-service or account-dependent limitation must be disclosed, and it prevents an unqualified 100% claim unless supported through authorized flows.

## 2026-11-06 — Foundation before renderer

The first implementation establishes testable world, entity, resource, and networking boundaries before selecting a renderer or platform abstraction.

## 2026-11-06 — Stream chunk persistence from contiguous storage

Chunk persistence reads and writes the existing contiguous block array directly, preserving the y/z/x serialized order while avoiding coordinate indexing and bounds checks for each block. This is a focused runtime optimization of the active world core and does not change the persistence format.

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

The inventory domain owns stack limits, item identity, player slots, equipment compatibility, cursor clicks, transfers, swaps, insertion, and consumption. UI code must call these operations rather than mutating slots directly. Exact transfers and swaps validate before changing either side; insertion may return an explicit remainder, and an inventory menu cannot discard an unreturned cursor stack. Save format v3 now persists the model; item components, catalog-specific validation, full reference slot semantics, world drops, crafting, and the in-game screen remain to be implemented and verified.

## 2026-11-07 â€” Persist bounded inventory state in save format v3

Player saves now encode 36 main slots, four armor slots, one offhand slot, item ID/variant/count, and selected hotbar index. The decoder enforces a fixed slot count, bounded stack counts, single-item armor slots, and a valid hotbar index. v1 and v2 saves migrate to empty inventory; their next save writes v3. Catalog-specific item identity/equipment validation must run when content definitions are available, and world-drop transactions are still separate work.

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
