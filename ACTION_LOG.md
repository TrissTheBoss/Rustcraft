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

## Rough timeline

- Baseline: complete, about 1 hour.
- v0.2: 1–2 sessions for robust argument parsing, platform command quoting, process logging, and integration tests.
- v0.3: 2–4 sessions for optional discovery of user-owned installations, only where documented and opt-in.
- 1.0: after cross-platform testing, security review, reproducible release builds, and user documentation.
