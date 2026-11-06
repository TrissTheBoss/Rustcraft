# Architecture decisions

## 2026-11-06 — Replace launcher product with clean-room engine

Rustcraft launcher and Fabric-trigger code were removed from the active tree after the product direction changed. Git history remains intact for auditability; the active product is now Voxel Engine.

## 2026-11-06 — No copied Minecraft implementation

The engine uses original code, original assets, and independently authored specifications. Existing Minecraft code, proprietary assets, authentication systems, and copied protocol implementations are out of scope.

## 2026-11-06 — Compatibility is a gated research track

Ordinary Minecraft-server compatibility and complete vanilla parity are not current features. They require public-specification study, legal review, conformance tests, and explicit evidence before any claim.

## 2026-11-06 — Foundation before renderer

The first implementation establishes testable world, entity, resource, and networking boundaries before selecting a renderer or platform abstraction.

## 2026-11-06 — Stream chunk persistence from contiguous storage

Chunk persistence reads and writes the existing contiguous block array directly, preserving the y/z/x serialized order while avoiding coordinate indexing and bounds checks for each block. This is a focused runtime optimization of the active world core and does not change the persistence format.

## 2026-11-06 — Fixed-step deterministic simulation

Simulation advances through an explicit fixed-step accumulator and stores bodies in a `BTreeMap` so tick order is stable by entity ID. The first collision boundary is an original ground plane at y=0; richer voxel collision remains a later increment.

## 2026-11-06 — Keep rendering core dependency-free until graphics dependencies are available

Implement the renderer-facing contracts and CPU rasterizer using the standard library so meshing, camera math, texture resources, lighting, and profiling can be exercised offline. The window/GPU adapter remains pending; do not represent the rendering milestone as complete until an interactive surface and GPU presentation path are integrated and verified.

## 2026-11-06 — Native Windows presentation before external graphics dependencies

The first interactive surface uses the Windows user32/gdi32 APIs directly and consumes the existing CPU renderer. This creates a testable windowed prototype without adding unavailable dependencies; it is a temporary presentation adapter, not the final cross-platform renderer.

## 2026-11-06 — Stream chunk persistence from contiguous storage

Chunk persistence reads and writes the existing contiguous block array directly, preserving the y/z/x serialized order while avoiding coordinate indexing and bounds checks for each block. This is a focused runtime optimization of the active world core and does not change the persistence format.
