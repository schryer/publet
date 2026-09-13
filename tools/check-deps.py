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
    "publet-graph": {"publet-core", "publet-lint"},
    "publet-eval": {"publet-core", "publet-graph"},
    "publet-merkle": {"publet-core"},
    "publet-domain": {"publet-core", "publet-merkle", "publet-graph"},
    "publet-lint": set(),
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
    """Workspace manifest graph, for the direct-edge check.

    Feature-independent and therefore fine for asking which workspace crates
    a crate names in its own manifest.
    """
    raw = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        capture_output=True, text=True, check=True).stdout
    return json.loads(raw)


def normal_deps(node):
    """Dependency ids reached by non-dev, non-build edges.

    Development dependencies do not ship and do not propagate to consumers,
    so a test harness using randomness -- proptest, whose whole job is
    randomness -- is not a purity violation.
    """
    out = []
    for dep in node["deps"]:
        kinds = dep.get("dep_kinds") or [{"kind": None}]
        if any(k.get("kind") is None for k in kinds):
            out.append(dep["pkg"])
    return out


def activated_transitive(crate):
    """Package names actually reachable from `crate` in a real build.

    `cargo metadata`'s resolve graph lists every dependency a package could
    have and ignores feature activation, so it reports crates that are never
    compiled -- ed25519-dalek names `rand_core` for key generation, which is
    off here, yet the graph shows it. `cargo tree` applies feature
    resolution, so it describes the artifact that actually ships. The guard
    should constrain what is built, not what a manifest could permit.
    """
    raw = subprocess.run(
        ["cargo", "tree", "-p", crate, "-e", "normal", "--prefix", "none",
         "--no-dedupe"],
        capture_output=True, text=True, check=True).stdout
    names = set()
    for line in raw.splitlines():
        line = line.strip()
        if not line or line.startswith("["):
            continue
        names.add(line.split()[0].removesuffix("*").strip())
    return names


def main():
    meta = metadata()
    name_of = {p["id"]: p["name"] for p in meta["packages"]}
    members_by_name = {name_of[i] for i in meta["workspace_members"] if i in name_of}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

    failures = []

    for pid, node in nodes.items():
        name = name_of[pid]
        if name not in ALLOWED:
            continue
        direct = {name_of[d] for d in normal_deps(node)}
        internal = {d for d in direct if d.startswith("publet-")}
        illegal = internal - ALLOWED[name]
        for dep in sorted(illegal):
            failures.append(f"{name} may not depend on {dep}")

    for crate, banned in FORBIDDEN_TRANSITIVE.items():
        if crate not in members_by_name:
            continue
        for dep in sorted(activated_transitive(crate) & banned):
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
