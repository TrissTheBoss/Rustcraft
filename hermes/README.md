# Hermes project agent

Hermes is a local workflow wrapper for this repository. `hermes-terminal.py` is only a prompt console. `hermes-worker.py` runs separately in the background and invokes the installed Codex CLI, so model execution and tool output do not occupy the interactive terminal.

## Authentication

Hermes delegates authentication to Codex CLI. Run `codex login` once and use the ChatGPT sign-in flow if your installed Codex build offers it. Hermes never reads, stores, or asks for ChatGPT passwords, cookies, or refresh tokens. API-key mode is optional and must use `OPENAI_API_KEY` supplied by the user.

## Start

Run `start-hermes.cmd`. It starts the worker hidden and opens a separate Hermes terminal. Use `install-hermes-shortcut.ps1` to create the taskbar shortcut.

## Safety

The worker uses workspace-write and approval-on-request defaults. Do not enable automatic full-disk or approval-bypass modes in the scripts.
