# Rustcraft Fabric trigger mod

This client-only Fabric mod is the in-game side of Rustcraft's local trigger protocol. It is designed for Minecraft 26.3 / Fabric Loader 0.20.1 and requires Fabric API. The default keybind is `J`; pressing it sends `RUSTCRAFT local-only` to `127.0.0.1:39281`.

The Rustcraft process must already be running in listener mode:

```text
rustcraft.exe --listen --config rustcraft.conf
```

The mod uses loopback only and contains no account, network-service, or file-discovery logic. Change the token in both the Rustcraft environment (`RUSTCRAFT_TRIGGER_TOKEN`) and the mod configuration before using a non-default token. Do not expose the listener beyond localhost.

This folder is currently a source scaffold. Before packaging, add the Fabric API version selected for the instance and build it with a pinned Gradle/Fabric Loom toolchain; do not commit the resulting jar or local instance files.
