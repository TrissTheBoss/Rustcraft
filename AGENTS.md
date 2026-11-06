# Voxel Engine contributor rules

The active product is a clean-room Rust voxel engine.

Use original code, assets, and independently authored specifications. A local-only extractor may read Minecraft assets from a user-owned local Java client JAR for that user's runtime, but extracted assets must never be committed, redistributed, downloaded from third parties, or used to bypass authentication or ownership checks. Do not copy proprietary Minecraft code or claim Minecraft compatibility or vanilla parity without documented evidence.

Before architectural work, inspect README.md, DECISIONS.md, and ACTION_LOG.md. Make focused changes, add tests, run formatting/tests/release builds, and document actions, results, decisions, failures, and next steps.

Never commit credentials, generated builds, local worlds, or external source checkouts. State assumptions and incomplete work clearly.
