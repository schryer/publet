#!/usr/bin/env python3
"""Every operating rule must be traceable to the code that keeps it.

The specification states sixteen rules. An implementation can enforce one
perfectly and never say so, and then a reader who starts from the rule
table has no way to reach the code -- which is a documentation failure that
no test detects, because the behaviour is correct.

Four rules were in that state before this guard existed: R2, R12, R13 and
R15 were enforced in cid.rs, publet-store, the settlement prohibitions and
the lineage views respectively, and cited by section number rather than by
rule, so none of them could be found from the rule they keep.

Tests are excluded deliberately. A test asserting a rule is evidence the
rule holds, not a pointer to where it is enforced, and counting them would
let a rule look traceable while nothing in the shipped code mentions it.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
RULES = range(1, 17)


def sources():
    for tree in ("crates", "bin", "porcelain"):
        for path in (ROOT / tree).rglob("*.rs"):
            parts = set(path.parts)
            if "target" in parts or "tests" in parts:
                continue
            yield path


def main() -> int:
    found: dict[int, list[str]] = {n: [] for n in RULES}
    for path in sources():
        text = path.read_text(encoding="utf-8", errors="replace")
        for raw in set(re.findall(r"\bR(1[0-6]|[1-9])\b", text)):
            found[int(raw)].append(str(path.relative_to(ROOT)))

    missing = [n for n in RULES if not found[n]]
    for n in RULES:
        where = found[n]
        mark = "!!" if not where else "  "
        detail = "no file cites it" if not where else f"{len(where)} file(s)"
        print(f"{mark} R{n:<3} {detail}")

    print()
    if missing:
        print("guard-rules: " + ", ".join(f"R{n}" for n in missing)
              + " are enforced by code that never names them.", file=sys.stderr)
        print("  Cite the rule where it is kept, so a reader starting from the",
              file=sys.stderr)
        print("  rule table can find the code. A section number is not enough:",
              file=sys.stderr)
        print("  sections say where a thing is specified, rules say what is owed.",
              file=sys.stderr)
        return 1

    print(f"guard-rules: clean (all {len(list(RULES))} rules traceable)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
