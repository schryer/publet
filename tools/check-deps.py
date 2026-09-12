#!/usr/bin/env python3
"""Enforce the dependency direction declared in the implementation plan.

`publet-eval` computes standing and must be a pure function of its inputs
(R8). If it can reach a store, a socket, a clock, or a random source, then
two conformant implementations can disagree and nothing will catch it in
review. The rule is therefore checked mechanically rather than trusted.
"""

import json
import subprocess
import sys

# crate -> crates it may depend on, within this workspace.
ALLOWED = {
    "publet-core": set(),
    "publet-graph": {"publet-core"},
    "publet-eval": {"publet-core", "publet-graph"},
    "publet-merkle": {"publet-core"},
    "publet-domain": {"publet-core", "publet-merkle", "publet-graph"},
    "publet-lint": {"publet-core"},
    "publet-store": {"publet-core", "publet-domain", "publet-merkle", "publet-graph"},
    "publet-settle": {"publet-core"},
    "publet-net": {"publet-core", "publet-domain", "publet-merkle",
                   "publet-graph", "publet-store"},
}

# Third-party crates that must never appear under publet-eval, at any depth.
FORBIDDEN_TRANSITIVE = {
    "publet-eval": {"tokio", "reqwest", "axum", "rand", "getrandom",
                    "chrono", "time", "redb", "rocksdb", "sled"},
}


def metadata():
    raw = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--all-features"],
        capture_output=True, text=True, check=True).stdout
    return json.loads(raw)


def main():
    meta = metadata()
    members = {i.split()[0] if " " in i else i.rsplit("#")[0].rsplit("/")[-1]
               for i in meta["workspace_members"]}
    by_id = {p["id"]: p for p in meta["packages"]}
    name_of = {p["id"]: p["name"] for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

    failures = []

    for pid, node in nodes.items():
        name = name_of[pid]
        if name not in ALLOWED:
            continue
        direct = {name_of[d] for d in node["dependencies"]}
        internal = {d for d in direct if d.startswith("publet-")}
        illegal = internal - ALLOWED[name]
        for dep in sorted(illegal):
            failures.append(f"{name} may not depend on {dep}")

    for crate, banned in FORBIDDEN_TRANSITIVE.items():
        pid = next((i for i, n in name_of.items() if n == crate), None)
        if pid is None:
            continue
        seen, stack = set(), [pid]
        while stack:
            cur = stack.pop()
            if cur in seen:
                continue
            seen.add(cur)
            stack.extend(d for d in nodes.get(cur, {}).get("dependencies", []))
        reached = {name_of[i] for i in seen} & banned
        for dep in sorted(reached):
            failures.append(
                f"{crate} reaches {dep} transitively; evaluation must stay pure")

    if failures:
        print("error: dependency direction violated", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1

    print(f"guard-deps: clean ({len(ALLOWED)} crates constrained)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
