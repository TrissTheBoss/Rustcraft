# Action log

This log records material repository actions, results, decisions, and next steps. Each milestone uses a focused commit, passes relevant checks, and receives a release tag. Generated files and local user data stay out of Git.

## 2026-11-06

- Re-scoped the repository from Rustcraft launcher/Fabric trigger to a clean-room Rust voxel engine at the user's direction. Result: launcher-specific runtime is no longer the active product. Next: build the engine foundation in incremental modules.
- Removed tracked launcher configuration, startup script, Fabric trigger project, and obsolete Cargo lockfile. Result: the active tree no longer presents a Minecraft launcher or mod as the product. Next: add original engine boundaries and documentation.
- Added original world/chunk storage, entity primitives, protocol-neutral session state, resource manifest types, and a minimal executable. Next: add deterministic simulation tests, renderer selection, and tooling interfaces.
- Documented the compatibility boundary: no Minecraft code/assets are included, and ordinary-server compatibility plus 100% parity are future acceptance targets rather than current claims.
- Expanded the roadmap into staged milestones from headless world core through simulation, rendering, tooling, original multiplayer, and release gates. Began `v0.2` implementation with world/chunk coordinates, a world manager, and dependency-free persistence. Next: finish the world core and its round-trip tests.
- Documented this conversation's durable project context in `CONVERSATION_MEMORY.md` and added `AGENTS.md` with clean-room, honesty, verification, documentation, handoff, and release rules. Result: future agents have repository-local instructions, while the file explicitly states that it cannot override higher-priority system or user instructions. Next: keep these files synchronized as the engine evolves.
- Added Hermes workflow scaffolding: separate terminal front end, hidden worker process, Codex CLI delegation, repository-local system contract, context-reduction guidance, GitHub Actions Rust CI, startup script, and Windows shortcut installer. Result: Hermes can use the locally authenticated Codex CLI without handling ChatGPT credentials itself. Next: run `codex login`, start Hermes, and verify one harmless repository-status request.

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
