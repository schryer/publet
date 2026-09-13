#!/usr/bin/env python3
"""Audit the annotation kinds and their fields against the implementation.

`must-coverage.py` measures RFC 2119 keyword statements. The object-shape
tables carry no keywords, so a field named there and never implemented is
invisible to it -- which is how `subjects` on `trusts` and three of the four
independence conditions went missing at 100% reported coverage.

The check is deliberately crude: it asks whether the field name appears as a
string literal anywhere in the library crates. That over-reports success --
a literal present for an unrelated reason counts as implemented -- so a
clean result here means little, while a miss is decisive. It is a lower
bound on what is absent.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]

# Annotation kind -> the value fields the specification names for it.
# Kinds whose table entry defers to a section have their fields taken from
# that section's object shape.
FIELDS = {
    "assessment": ["verdict", "basis"],
    "verdict": ["finding", "aspect", "method", "effort"],
    "proof-checked": ["system", "version", "artifact", "result"],
    "reproduction": ["outcome", "independence", "funding", "shared_materials",
                     "result", "data"],
    "classifies": ["subject"],
    "critique": ["defect", "omitted"],
    # The specification says only "corpus evidence" and never gives a
    # shape, so this one is a design decision rather than a reading: a
    # citation names where a sense is used. That gap should be closed in
    # the specification rather than left implied by an implementation.
    "usage": ["source", "locator", "sense"],
    "resolution": ["thread_root", "outcome", "policy", "snapshot"],
    "trusts": ["weight", "subjects"],
    "attests": ["identity"],
    "affiliated": ["organization", "role", "period"],
    "personhood": ["scheme", "issuer_set", "scope", "nullifier", "proof",
                   "anonymity"],
    "assumes-accountability": ["basis"],
    "timestamped": ["at", "service", "proof"],
    "triage": ["finding", "engine"],
    "well-formed": ["test"],
    "witnessed": ["root", "size"],
}


def sources() -> str:
    """Every Rust source in the workspace.

    Reading a field is not confined to the library crates -- a porcelain
    command that renders one reads it too -- so scanning only `crates/`
    reports fields as unread that are merely read elsewhere.
    """
    out = []
    for tree in ("crates", "bin", "porcelain"):
        for path in sorted((ROOT / tree).glob("**/*.rs")):
            out.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(out)


def main() -> int:
    blob = sources()
    literals = set(re.findall(r'"([A-Za-z0-9_-]+)"', blob))

    missing_kinds, missing_fields = [], []
    for kind, fields in sorted(FIELDS.items()):
        if kind not in literals:
            missing_kinds.append(kind)
        for field in fields:
            if field not in literals:
                missing_fields.append(f"{kind}.{field}")

    width = max(len(k) for k in FIELDS)
    for kind, fields in sorted(FIELDS.items()):
        absent = [f for f in fields if f not in literals]
        mark = "--" if kind not in literals else ("  " if not absent else "!!")
        detail = ""
        if kind not in literals:
            detail = "kind never read"
        elif absent:
            detail = "fields absent: " + ", ".join(absent)
        print(f"{mark} {kind:<{width}}  {detail}")

    print()
    print(f"annotation kinds never read: {len(missing_kinds)}/{len(FIELDS)}")
    print(f"declared fields never read:  {len(missing_fields)}")
    if missing_kinds:
        print("  kinds : " + ", ".join(missing_kinds))
    if missing_fields:
        print("  fields: " + ", ".join(missing_fields))

    # Reports, never fails: this is a map of what is left, not a gate.
    return 0


if __name__ == "__main__":
    sys.exit(main())
