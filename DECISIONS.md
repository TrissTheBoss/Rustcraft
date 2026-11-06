# Architecture decisions

## 2026-11-06 — Replace launcher product with clean-room engine

Rustcraft launcher and Fabric-trigger code were removed from the active tree after the product direction changed. Git history remains intact for auditability; the active product is now Voxel Engine.

## 2026-11-06 — No copied Minecraft implementation

The engine uses original code, original assets, and independently authored specifications. Existing Minecraft code, proprietary assets, authentication systems, and copied protocol implementations are out of scope.

## 2026-11-06 — Compatibility is a gated research track

Ordinary Minecraft-server compatibility and complete vanilla parity are not current features. They require public-specification study, legal review, conformance tests, and explicit evidence before any claim.

## 2026-11-06 — Foundation before renderer

The first implementation establishes testable world, entity, resource, and networking boundaries before selecting a renderer or platform abstraction.
