# Action log

This log records material repository actions, results, decisions, and next steps. Each milestone uses a focused commit, passes relevant checks, and receives a release tag. Generated files and local user data stay out of Git.

## 2026-11-06

- Re-scoped the repository from Rustcraft launcher/Fabric trigger to a clean-room Rust voxel engine at the user's direction. Result: launcher-specific runtime is no longer the active product. Next: build the engine foundation in incremental modules.
- Removed tracked launcher configuration, startup script, Fabric trigger project, and obsolete Cargo lockfile. Result: the active tree no longer presents a Minecraft launcher or mod as the product. Next: add original engine boundaries and documentation.
- Added original world/chunk storage, entity primitives, protocol-neutral session state, resource manifest types, and a minimal executable. Next: add deterministic simulation tests, renderer selection, and tooling interfaces.
- Documented the compatibility boundary: no Minecraft code/assets are included, and ordinary-server compatibility plus 100% parity are future acceptance targets rather than current claims.

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
