#!/usr/bin/env python3
"""Every public function of a published crate has an example, and every
public item says which tests cover it.

For each published crate this:

- lists its public functions and methods (`pub fn`; the workspace's
  `unreachable_pub` lint makes every `pub` item reachable) and fails if
  any has no example in its documentation;
- reads `// covers: Item` comments in the crate's tests -- a test names
  the items it exercises, such as `// covers: Object::parse, Cid::of` --
  and fails if one names an item that does not exist;
- writes the crate's TESTING.md: for every public item, how many examples
  its documentation runs and which tests cover it.

TESTING.md is a view, never edited by hand. `make doc-coverage` writes it;
`make doc-coverage-check` (in CI) fails if it is stale or the rules above
are broken.

Usage: tools/doc-coverage.py [--check]
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PUBLISHED = ["publet-core", "publet-algorithms"]
REPO = "https://github.com/schryer/publet/blob/main"

FN = re.compile(r"\s*pub (?:const )?fn (\w+)")
TYPE = re.compile(r"\s*pub (?:struct|enum|trait|type) (\w+)")
IMPL = re.compile(r"\s*impl(?:<[^>]*>)?\s+(?:[\w:<>, ]+\s+for\s+)?(\w+)")
TEST_FN = re.compile(r"\s*(?:pub )?fn (\w+)")
COVERS = re.compile(r"//\s*covers:\s*(.+)")


def strip(line):
    """The line without string literals or a trailing comment, for brace counting."""
    line = re.sub(r'"(?:\\.|[^"\\])*"', '""', line)
    line = re.sub(r"'(?:\\.|[^'\\])'", "''", line)
    return line.split("//", 1)[0]


def public_module(src, path):
    """The module path users write for items defined in `path`.

    Only modules declared `pub mod` appear: a private module's items are
    re-exported into its parent (`mod sig; pub use sig::sign;` makes
    `sign` a crate-root function), so they are named from there.
    """
    rel = path.relative_to(src).with_suffix("")
    parts = [p for p in rel.parts if p not in ("lib", "mod")]
    chain, parent = [], src / "lib.rs"
    for part in parts:
        declared = parent.read_text() if parent.exists() else ""
        if re.search(rf"^\s*pub mod {part}\s*;", declared, re.M):
            chain.append(part)
        directory = parent.parent if parent.name in ("lib.rs", "mod.rs") else parent.with_suffix("")
        parent = directory / part / "mod.rs"
        if not parent.exists():
            parent = directory / f"{part}.rs"
    return "::".join(chain)


def items(crate):
    """Public items of `crate`: (name, kind, file, line, examples)."""
    out = []
    src = ROOT / "crates" / crate / "src"
    for path in sorted(src.rglob("*.rs")):
        lines = path.read_text().splitlines()
        depth, impl_stack, in_test = 0, [], None
        module = public_module(src, path)
        for i, line in enumerate(lines):
            if in_test is None and re.match(r"\s*#\[cfg\(test\)\]", line):
                in_test = depth
            code = strip(line)
            if in_test is None:
                impl = IMPL.match(line) if line.strip().startswith("impl") else None
                fn, ty = FN.match(line), TYPE.match(line)
                if fn or ty:
                    j, doc = i - 1, []
                    while j >= 0 and (lines[j].strip().startswith("///")
                                      or lines[j].strip().startswith("#[")):
                        doc.append(lines[j])
                        j -= 1
                    examples = sum(1 for d in doc if d.strip().startswith("/// ```")) // 2
                    if fn:
                        owner = impl_stack[-1][1] if impl_stack else module
                        name = f"{owner}::{fn.group(1)}" if owner else fn.group(1)
                        out.append((name, "fn", path, i + 1, examples))
                    else:
                        out.append((ty.group(1), "type", path, i + 1, examples))
                if impl and "{" in code:
                    impl_stack.append((depth, impl.group(1)))
            depth += code.count("{") - code.count("}")
            while impl_stack and depth <= impl_stack[-1][0]:
                impl_stack.pop()
            if in_test is not None and depth <= in_test and "}" in code:
                in_test = None
    return out


def covered(crate):
    """Tests that say what they cover: {item: [(file, test, line)]}."""
    found = {}
    base = ROOT / "crates" / crate
    for path in sorted(list((base / "tests").rglob("*.rs")) + list((base / "src").rglob("*.rs"))):
        lines = path.read_text().splitlines()
        for i, line in enumerate(lines):
            m = COVERS.search(line)
            if not m:
                continue
            test = None
            for later in lines[i + 1:i + 8]:
                t = TEST_FN.match(later)
                if t:
                    test = t.group(1)
                    break
            for name in (n.strip() for n in m.group(1).split(",")):
                if name:
                    found.setdefault(name, []).append((path.relative_to(ROOT), test, i + 1))
    return found


def report(crate, public, tests):
    fns = [p for p in public if p[1] == "fn"]
    with_examples = sum(1 for p in fns if p[4] > 0)
    with_tests = sum(1 for p in public if p[0] in tests)
    lines = [
        f"# Testing: {crate}",
        "",
        "Generated by `tools/doc-coverage.py`; do not edit.",
        "",
        f"{len(public)} public items; {with_examples} of {len(fns)} functions have examples, "
        f"and {with_tests} items name the tests that cover them.",
        "",
        "Every example in the documentation runs as a test (`cargo test --doc`). The",
        "tests named here say what they cover with a `// covers:` comment; the files",
        "are linked below.",
        "",
        "| Item | Examples | Tests |",
        "|---|---|---|",
    ]
    for name, kind, path, line, examples in sorted(public, key=lambda p: p[0].lower()):
        names = sorted({f"`{t}`" for _, t, _ in tests.get(name, []) if t})
        lines.append(f"| `{name}` | {examples if kind == 'fn' or examples else '-'} | "
                     f"{', '.join(names) if names else '-'} |")
    files = sorted({str(f) for v in tests.values() for f, _, _ in v})
    if files:
        lines += ["", "Test files:", ""]
        lines += [f"- [`{f}`]({REPO}/{f})" for f in files]
    return "\n".join(lines) + "\n"


def main():
    check = "--check" in sys.argv[1:]
    problems = []
    for crate in PUBLISHED:
        public = items(crate)
        names = {p[0] for p in public}
        tests = covered(crate)
        for p in public:
            if p[1] == "fn" and p[4] == 0:
                problems.append(f"{p[2].relative_to(ROOT)}:{p[3]}: `{p[0]}` has no example")
        for name, sites in tests.items():
            if name not in names:
                for f, _, line in sites:
                    problems.append(f"{f}:{line}: covers `{name}`, which is not a public item of {crate}")
        text = report(crate, public, tests)
        path = ROOT / "crates" / crate / "TESTING.md"
        if check:
            if not path.exists() or path.read_text() != text:
                problems.append(f"{path.relative_to(ROOT)} is stale; run `make doc-coverage`")
        else:
            path.write_text(text)
            print(f"wrote {path.relative_to(ROOT)}")
    if problems:
        print("\n".join(problems), file=sys.stderr)
        sys.exit(1 if check else 0)


if __name__ == "__main__":
    main()
