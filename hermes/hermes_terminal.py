from pathlib import Path
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parent
INBOX = ROOT / "inbox"
INBOX.mkdir(exist_ok=True)

print("Hermes terminal. Messages are handled by the background worker. Type /exit to close.")
while True:
    try:
        message = input("hermes> ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        break
    if not message:
        continue
    if message.lower() in {"/exit", "/quit"}:
        break
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    (INBOX / f"{stamp}.prompt").write_text(message + "\n", encoding="utf-8")
    print("Queued. Hermes is working in the background; responses appear in hermes/outbox.")
