#!/usr/bin/env python3
"""Report which normative statements the functional suite claims to cover.

Every scenario exercising a MUST or MUST NOT carries a `@must-<section>`
tag. This maps those tags against the specification and names the sections
with normative language and no scenario. The gaps are the point: they are
published with each release rather than hidden.

    ./tools/must-coverage.py [--spec PATH] [--fail-under N]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SPEC = ROOT.parent / "docs" / "publet-specification" / "index.md"

NORMATIVE = re.compile(r"\b(MUST NOT|MUST|SHALL NOT|SHALL|REQUIRED)\b")
HEADING = re.compile(r"^#{2,4} (\d+(?:\.\d+)*)\s")


def tagged_sections(features: Path) -> dict[str, list[str]]:
    """Map section number -> feature files claiming to cover it."""
    out: dict[str, list[str]] = {}
    for path in sorted(features.rglob("*.feature")):
        for line in path.read_text().splitlines():
            stripped = line.strip()
            if not stripped.startswith("@"):
                continue
            for tag in stripped.split():
                if tag.startswith("@must-"):
                    out.setdefault(tag[len("@must-"):], []).append(
                        str(path.relative_to(features))
                    )
    return out


def normative_sections(spec: Path) -> dict[str, int]:
    """Map section number -> count of normative statements in it."""
    counts: dict[str, int] = {}
    current = None
    for line in spec.read_text().splitlines():
        heading = HEADING.match(line)
        if heading:
            current = heading.group(1)
            continue
        if current and NORMATIVE.search(line):
            counts[current] = counts.get(current, 0) + 1
    return counts


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    ap.add_argument("--fail-under", type=int, default=0,
                    help="exit non-zero if coverage is below this percentage")
    args = ap.parse_args()

    covered = tagged_sections(ROOT / "tests" / "features")

    if not args.spec.exists():
        print(f"specification not found at {args.spec}")
        print(f"tagged sections: {', '.join(sorted(covered)) or 'none'}")
        print("pass --spec to cross-check against the specification")
        return 0

    normative = normative_sections(args.spec)
    done = sorted(s for s in normative if s in covered)
    todo = sorted(s for s in normative if s not in covered)
    stray = sorted(s for s in covered if s not in normative)

    total = len(normative)
    percent = (len(done) * 100 // total) if total else 100

    print(f"normative sections: {total}")
    print(f"covered:            {len(done)} ({percent}%)")
    print()
    if done:
        print("covered")
        for s in done:
            print(f"  {s:<8} {normative[s]:>2} statement(s)  <- {', '.join(covered[s])}")
        print()
    if todo:
        print("not yet covered")
        for s in todo:
            print(f"  {s:<8} {normative[s]:>2} statement(s)")
        print()
    if stray:
        print("tags naming sections with no normative language:")
        for s in stray:
            print(f"  {s}")
        print()

    if percent < args.fail_under:
        print(f"coverage {percent}% is below the required {args.fail_under}%")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
