#!/usr/bin/env python3
"""Internal-consistency audit for index.md (the PUB specification).

This document cross-references itself heavily: numbered sections, rule IDs
(R1..Rn), assumed-environment IDs (L1..Ln), open-problem IDs (B.n), and an
IANA registry that must list every object type the text actually uses. Past
that scale, consistency does not survive hand-editing - every check here
corresponds to a real defect found after the fact.

Usage:  ./audit.py [path-to-index.md]     (exit 0 clean, 1 on findings)
"""

import re
import sys
from pathlib import Path

NUMBER_WORDS = {
    "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7,
    "eight": 8, "nine": 9, "ten": 10, "eleven": 11, "twelve": 12,
    "thirteen": 13, "fourteen": 14, "fifteen": 15, "sixteen": 16,
    "seventeen": 17, "eighteen": 18, "nineteen": 19, "twenty": 20,
}


def audit(text):
    """Return a list of finding strings; empty means clean."""
    out = []

    def check(label, bad, fmt="{}"):
        if bad:
            out.append("%s: %s" % (label, ", ".join(fmt.format(b) for b in bad)))

    # --- anchors defined by the document -------------------------------
    # Section targets are markdown headings; the IANA registries are table
    # rows ("| 18.2 | ...") rather than headings, so both count as targets.
    sections = {m.group(1) for m in re.finditer(r"^#{2,4} (\d+(?:\.\d+)*)", text, re.M)}
    sections |= {m.group(1) for m in re.finditer(r"^\| (\d+\.\d+) \|", text, re.M)}
    rules = {m.group(1) for m in re.finditer(r"^\| \*\*(R\d+)\*\*", text, re.M)}
    assumptions = {m.group(1) for m in re.finditer(r"^\| \*\*(L\d+)\*\*", text, re.M)}
    problems = {m.group(1) for m in re.finditer(r"^\*\*(B\.\d+)", text, re.M)}

    # --- 1. dangling cross-references ----------------------------------
    refs = set(re.findall(r"Section (\d+(?:\.\d+)*)", text))
    check("dangling Section refs", sorted(refs - sections))

    check("dangling rule refs", sorted(
        {"R" + n for n in re.findall(r"\bR(\d+)\b", text)} - rules))

    check("dangling assumption refs", sorted(
        {"L" + n for n in re.findall(r"\bL(\d)\b", text)} - assumptions))

    b_refs = set(re.findall(r"Appendix (B\.\d+)", text))
    b_refs |= set(re.findall(r"\((B\.\d+)", text))
    b_refs |= set(re.findall(r", (B\.\d+)\)", text))
    check("dangling open-problem refs", sorted(b_refs - problems))

    # --- 2. ordering of numbered lists ---------------------------------
    # Appending an entry out of order is easy and reads as a defect.
    for label, ids, key in (
        ("rules", rules, lambda s: int(s[1:])),
        ("open problems", problems, lambda s: int(s[2:])),
    ):
        order = [m for m in re.findall(
            r"^\| \*\*(R\d+)\*\*" if label == "rules" else r"^\*\*(B\.\d+)",
            text, re.M)]
        if order != sorted(order, key=key):
            out.append("%s are out of order in the document: %s"
                       % (label, " ".join(order)))
        nums = sorted(key(i) for i in ids)
        gaps = [n for n in range(1, (nums[-1] if nums else 0) + 1) if n not in nums]
        check("%s: gaps in numbering" % label, gaps)

    # --- 3. stale counts in prose --------------------------------------
    # "these fourteen rules" after a fifteenth is added.
    # Only phrasings that mean the operating-rule set, not e.g. "bound by
    # three rules" about some local list.
    count_re = r"\b(?:these |stated as )(%s) (?:operating )?rules\b|\b(%s) operating rules\b" % (
        "|".join(NUMBER_WORDS), "|".join(NUMBER_WORDS))
    for m in re.finditer(count_re, text, re.I):
        word = m.group(1) or m.group(2)
        if NUMBER_WORDS[word.lower()] != len(rules):
            out.append('prose says "%s ... rules" but %d rules are defined'
                       % (word, len(rules)))

    # --- 4. table of contents vs headings ------------------------------
    toc_block = re.search(r"^## Table of Contents\n(.*?)^---", text, re.M | re.S)
    if toc_block:
        toc = re.findall(r"^(\d+)\. (.+)$", toc_block.group(1), re.M)
        heads = re.findall(r"^## (\d+)\. (.+)$", text, re.M)
        if toc != heads:
            for a, b in zip(toc, heads):
                if a != b:
                    out.append("ToC/heading mismatch: %s. %r vs %r" % (a[0], a[1], b[1]))
            if len(toc) != len(heads):
                out.append("ToC lists %d sections, document has %d"
                           % (len(toc), len(heads)))
    else:
        out.append("no Table of Contents block found")

    # --- 5. IANA object-type registry vs types used --------------------
    row = re.search(r"Object types \| (.+?) \|\n", text)
    if row:
        registered = set(re.findall(r"`([a-z-]+)`", row.group(1)))
        used = set(re.findall(r'type: "([a-z-]+)"', text))
        check("object types used but not registered", sorted(used - registered))
        # Types defined only in prose are fine; report for eyeballing only.
        unused = sorted(t for t in registered - used
                        if not re.search(r"`%s` object" % re.escape(t), text))
        check("registered object types with no definition", unused)
    else:
        out.append("no IANA object-type row found")

    # --- 6. annotation and relation kinds ------------------------------
    for label, pattern, registry in (
        ("annotation kinds", r'kind: "([a-z-]+)",\s*\n?\s*target:', "Annotation kinds"),
        ("relation kinds", r"^\| `([a-z-]+)` \| `from`", "Relation kinds"),
    ):
        row = re.search(r"%s \| (.+?) \|\n" % registry, text)
        if not row:
            out.append("no IANA row for %s" % registry)
            continue
        registered = set(re.findall(r"`([a-z-]+)`", row.group(1)))
        used = set(re.findall(pattern, text, re.M))
        check("%s used but not registered" % label, sorted(used - registered))

    return out


def main():
    path = Path(sys.argv[1] if len(sys.argv) > 1
                else Path(__file__).with_name("index.md"))
    findings = audit(path.read_text())
    if findings:
        print("%s: %d finding(s)\n" % (path.name, len(findings)))
        for f in findings:
            print("  - %s" % f)
        return 1
    print("%s: consistent" % path.name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
