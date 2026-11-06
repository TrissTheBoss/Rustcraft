import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parent
INBOX = ROOT / "inbox"
OUTBOX = ROOT / "outbox"
STATE = ROOT / "state"
for folder in (INBOX, OUTBOX, STATE):
    folder.mkdir(exist_ok=True)

SYSTEM = (ROOT / "SYSTEM.md").read_text(encoding="utf-8")
CONTEXT = PROJECT / "CONVERSATION_MEMORY.md"

def run_prompt(prompt_file: Path) -> None:
    prompt = prompt_file.read_text(encoding="utf-8")
    context = CONTEXT.read_text(encoding="utf-8") if CONTEXT.exists() else ""
    request = f"{SYSTEM}\n\nPROJECT MEMORY:\n{context}\n\nUSER REQUEST:\n{prompt}"
    output = OUTBOX / (prompt_file.stem + ".response.txt")
    command = ["codex", "exec", "-C", str(PROJECT), "-s", "workspace-write", "-a", "on-request", request]
    with output.open("w", encoding="utf-8") as stream:
        result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, text=True, check=False)
        stream.write(f"\n\n[Hermes exit code: {result.returncode}]\n")
    prompt_file.rename(STATE / prompt_file.name)

while True:
    for prompt_file in sorted(INBOX.glob("*.prompt")):
        try:
            run_prompt(prompt_file)
        except Exception as error:
            (OUTBOX / (prompt_file.stem + ".error.txt")).write_text(str(error), encoding="utf-8")
            prompt_file.rename(STATE / prompt_file.name)
    time.sleep(1)
