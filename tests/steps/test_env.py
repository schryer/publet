"""Steps for environment publets: `pub env` and rendering inside one.

Provisioning here fetches nothing over the network: a stub host program,
`stubfetch`, "downloads" known bytes and installs a tool, `envtool`, that
exists only inside the environment. `stubnode` stands for a runtime host
requirement such as node.
"""

from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import cid_of

scenarios("../features/porcelain/env.feature")

PAYLOAD = b"payload"

STUBFETCH = """#!/bin/sh
case "$1" in
  --version) echo "stubfetch {version}" ;;
  get) mkdir -p "$(dirname "$2")"; printf 'payload' > "$2" ;;
  install)
    mkdir -p "$(dirname "$2")"
    cat > "$2" <<'TOOL'
#!/bin/sh
case "$1" in
  --version) echo "envtool 1.0.0" ;;
  write) echo "rendered $3" > "$2" ;;
  *) exit 2 ;;
esac
TOOL
    chmod +x "$2" ;;
  *) exit 2 ;;
esac
"""

STUBNODE = """#!/bin/sh
echo "stubnode {version}"
"""


def table(columns, rows):
    return {"columns": [{"name": c} for c in columns], "rows": rows}


def sources(pin: str) -> dict:
    return {
        "env.stub": {
            "tag": "ENV-STUBEN-10-2026", "title": "Stub environment",
            "claims": {
                "host": {
                    "class": "archival", "scope": "the test host",
                    "content": "Provisioning needs stubfetch; running needs stubnode.",
                    "data": table(["name", "requires", "check", "when"], [
                        ["stubfetch", "1.0", "stubfetch --version", "provision"],
                        ["stubnode", "1.0", "stubnode --version", "runtime"],
                    ]),
                    "sources": ["https://example.org/host"],
                },
                "provision": {"class": "procedural", "scope": "the test",
                              "content": "To provision, fetch the payload, then install the tool.",
                              "depends": ["#fetch", "#install"]},
                "fetch": {
                    "class": "procedural", "scope": "the test",
                    "content": "Fetch the payload.",
                    "data": table(["program", "args", "cwd", "outputs"], [
                        ["stubfetch", "get {env}/downloads/payload.txt", "{env}",
                         f"downloads/payload.txt@{pin}"]]),
                    "sources": ["https://example.org/repo"],
                },
                "install": {
                    "class": "procedural", "scope": "the test",
                    "content": "Install the tool into the environment.",
                    "data": table(["program", "args", "cwd", "outputs"], [
                        ["stubfetch", "install {env}/bin/envtool", "{env}", "bin/envtool"]]),
                    "sources": ["https://example.org/repo"],
                },
            },
            "sections": [{"heading": "Provision", "items": [{"ref": "#provision"}]},
                         {"heading": "Host", "items": [{"ref": "#host"}]}],
        },
        "tool.envtool": {
            "tag": "TOOL-ENVTLS-10-2026", "title": "envtool",
            "claims": {"identity": {
                "class": "archival", "scope": "the test",
                "content": "envtool 1.0.0 is the program the pipeline runs.",
                "data": table(["name", "version", "check", "expect"],
                              [["envtool", "1.0.0", "envtool --version", "envtool 1.0.0"]]),
                "sources": ["https://example.org/envtool"],
            }},
            "sections": [{"heading": "Identity", "items": [{"ref": "#identity"}]}],
        },
        "render.env": {
            "tag": "RENDER-ENVPLN-10-2026", "title": "Pipeline in an environment",
            "claims": {
                "method": {"class": "procedural", "scope": "the test",
                           "content": "To render, run its one step.", "depends": ["#write"]},
                "write": {
                    "class": "procedural", "scope": "the test",
                    "content": "Write the result with envtool.",
                    "depends": ["~tool.envtool#identity"],
                    "data": table(["program", "requires", "args", "cwd", "outputs"], [
                        ["envtool", "1.0", "write {out}/result.txt {doc}", "{out}",
                         "result.txt"]]),
                    "sources": ["https://example.org/repo"],
                },
            },
            "sections": [
                {"heading": "Method", "items": [{"ref": "#method"}]},
                {"heading": "Environment", "items": [
                    {"ref": "env.stub", "role": "background", "bind": "lineage"}]},
            ],
        },
        "demo-doc": {
            "tag": "PUB-DEMODC-10-2026", "title": "Demo document",
            "sections": [{"heading": "Colophon", "items": [
                {"ref": "render.env", "role": "background", "bind": "lineage"}]}],
        },
    }


class Env:
    def __init__(self, runner):
        self.runner = runner
        self.root = runner.cwd / "corpus"
        self.bin = runner.cwd / "host-bin"
        self.cache = runner.cwd / "cache"

    def env(self) -> dict:
        return dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}",
                    XDG_CACHE_HOME=str(self.cache))

    def pub(self, *args: str):
        return subprocess.run([str(self.runner.path_to("pub")), *args], capture_output=True,
                              text=True, cwd=self.root, env=self.env(), check=False)

    def ok(self, *args: str):
        proc = self.pub(*args)
        assert proc.returncode == 0, proc.stdout + proc.stderr
        return proc

    def script(self, name: str, text: str):
        path = self.bin / name
        path.write_text(text)
        path.chmod(path.stat().st_mode | stat.S_IEXEC)

    def write(self, pin: str):
        for slug, body in sources(pin).items():
            path = self.root / "publets" / slug / "publet.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"slug": slug, **body}, indent=2))

    def env_cid(self) -> str:
        rows = json.loads((self.root / ".publet" / "drafts.json").read_text())["publets"]
        return next(r["cid"] for r in rows if r["slug"] == "env.stub")

    def records(self) -> list[Path]:
        return list(self.cache.rglob("environment.json"))


@given("a corpus whose pipeline runs a tool its environment provisions", target_fixture="env")
def corpus(runner):
    e = Env(runner)
    e.root.mkdir()
    e.bin.mkdir()
    e.script("stubfetch", STUBFETCH.format(version="1.0.0"))
    e.script("stubnode", STUBNODE.format(version="1.0.0"))
    e.ok("corpus", "init", "--name=test")
    key = e.ok("sign", "--generate-key", "--principal=human").stdout.split()[1]
    e.ok("policy", f"--root={key}")
    e.write(cid_of(PAYLOAD))
    return e


@when("I build and render the document", target_fixture="result")
def build_render(env):
    env.ok("build")
    return env.pub("render", "demo-doc")


@when("I build and provision the environment", target_fixture="result")
def build_provision(env):
    env.ok("build")
    return env.pub("env", "build", "env.stub")


@when("I provision the environment again", target_fixture="result")
def provision_again(env):
    return env.ok("env", "build", "env.stub")


@when("I render the document", target_fixture="result")
def render(env):
    return env.pub("render", "demo-doc")


@when(parsers.parse('the host lacks "{name}"'))
def host_lacks(env, name: str):
    (env.bin / name).unlink()


@when(parsers.parse('the host\'s "{name}" reports version "{version}"'))
def host_version(env, name: str, version: str):
    template = STUBFETCH if name == "stubfetch" else STUBNODE
    env.script(name, template.format(version=version))


@when("the environment pins the fetched file to other bytes")
def bad_pin(env):
    env.write(cid_of(b"other bytes"))


@when("I build and ask for the environment's path", target_fixture="result")
def ask_path(env):
    env.ok("build")
    return env.ok("env", "path", "env.stub")


@then("the tool is not on the host's PATH")
def not_on_host(env):
    assert shutil.which("envtool", path=env.env()["PATH"]) is None


@then("the document's output names the document")
def output_names_doc(env):
    out = (env.root / "publets" / "demo-doc" / "result.txt").read_text()
    rows = json.loads((env.root / ".publet" / "drafts.json").read_text())["publets"]
    doc = next(r["cid"] for r in rows if r["slug"] == "demo-doc")
    assert out.strip() == f"rendered {doc}"


@then("the environment is not provisioned")
def not_provisioned(env):
    assert env.records() == []


@then(parsers.parse('it reports "{text}"'))
def reports(result, text: str):
    assert text in result.stdout, result.stdout + result.stderr


@then("the path ends with the environment's identifier")
def path_named(env, result):
    assert result.stdout.strip().endswith(env.env_cid().replace(":", "_"))
