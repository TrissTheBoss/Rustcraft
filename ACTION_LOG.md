# Action log and operating procedure

Each entry records date, action, result, and next step. Changes should be made in focused commits, tested locally, documented here, and tagged when a usable build exists. Keep generated binaries, Minecraft data, secrets, and personal configs out of Git.

## 2026-11-05

- Inspected Horizon's public repository and MIT license; result: license permits reuse with notices, but ownership/authentication constraints remain. Next: use only documented behavior or separately verified compatible code.
- Checked Rustcraft's GitHub URL; result: no visible refs were returned. Next: initialize the local repository and publish when GitHub credentials permit.
- Created the v0.1.0 clean-room baseline; result: minimal immediate-start CLI with explicit configuration. Next: run formatting, tests, and smoke launch.
- Attempted local verification; result: blocked because `cargo` is not installed or on PATH. Next: install Rust/Cargo or run CI before publishing a binary release.
- Installed Rust stable 1.99.1 through rustup. `cargo fmt` passes; compilation then found the Windows MSVC linker was missing.
- Started Visual Studio 2022 Build Tools with the C++ workload through winget. The installer was still active during this session, so `cargo test` remains pending until `link.exe` is available.
- Verified the installation completed: MSVC `link.exe` is available from Visual Studio 2022 Build Tools. `cargo fmt --check`, `cargo test`, and `cargo run -- --help` all pass. Tagged this verified state as `v0.1.1`.
- Ran the release binary with `rustcraft.conf.example`; it launched Java 21 successfully. Fixed argument quote handling discovered during that run, then reran `cargo fmt --check`, `cargo test`, and the release smoke test successfully. Tagged `v0.1.2`.
- Located the Prism instance `Rustcraft testing` in the `Rustcraft` group. Added quote-aware argument parsing, then launched Prism through Rustcraft with `--launch "Rustcraft testing"`; Prism accepted the exact instance ID and prepared Minecraft 26.3. No Java process remained at the follow-up check, indicating an instance-side startup/authentication exit rather than a Rustcraft handoff failure. Tagged `v0.1.3`.
- Added the loopback trigger protocol and a Fabric client-mod source scaffold for Minecraft 26.3 / Fabric Loader 0.20.1. Rustcraft now supports `--listen`; the mod sends a tokenized trigger from key `J`. Rust tests pass. Fabric API dependency/build packaging remains the next step because the converted instance does not yet expose a selected Fabric API version.
- Selected official Fabric API `0.162.0+26.3` and installed Gradle 9.8.2 for Loom 1.19.3. Build investigation found the 26.3 non-obfuscated Loom setup cannot use the Prism-cached client jar as a Maven-style Minecraft dependency; no unverified jar was installed. Next: use the official 26.3 non-obfuscated template/dependency coordinates or target a published Fabric API template once available.

## Rough timeline

- Baseline: complete, about 1 hour.
- v0.2: 1–2 sessions for robust argument parsing, platform command quoting, process logging, and integration tests.
- v0.3: 2–4 sessions for optional discovery of user-owned installations, only where documented and opt-in.
- 1.0: after cross-platform testing, security review, reproducible release builds, and user documentation.
