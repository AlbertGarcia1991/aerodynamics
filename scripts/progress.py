#!/usr/bin/env python3
"""Append a milestone entry to docs/IMPLEMENTATION_PROGRESS.md and update the ledger.

Usage: scripts/progress.py <percent> <title> <<'MD'
...markdown body...
MD
"""
import sys, re, pathlib, datetime

DOC = pathlib.Path(__file__).resolve().parent.parent / "docs" / "IMPLEMENTATION_PROGRESS.md"

def main() -> int:
    if len(sys.argv) < 3:
        print("usage: progress.py <percent> <title>", file=sys.stderr)
        return 2
    pct, title = int(sys.argv[1]), sys.argv[2]
    body = sys.stdin.read().rstrip()
    text = DOC.read_text()

    # 1. Headline percentage.
    text = re.sub(r"\| Current progress \| \*\*\d+%\*\* \|",
                  f"| Current progress | **{pct}%** |", text)

    # 2. Tick every ledger row at or below this percentage.
    def tick(m):
        row_pct = int(m.group(1))
        return m.group(0).replace("⬜", "✅ done") if row_pct <= pct else m.group(0)
    text = re.sub(r"\| (\d+) \| [^|]*\| ⬜ \|", tick, text)

    today = datetime.date.today().isoformat()
    entry = f"\n---\n\n## {pct}% — {title}\n\n**Date:** {today}\n\n{body}\n"
    DOC.write_text(text.rstrip() + "\n" + entry)
    print(f"progress -> {pct}%  ({title})")
    return 0

if __name__ == "__main__":
    sys.exit(main())
