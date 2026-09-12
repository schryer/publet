"""Steps for the conformance features that need no workspace.

These exercise the properties a second implementation must also have, so
they are deliberately expressed against the binaries and the committed
vectors rather than against anything internal.
"""

from __future__ import annotations

from pathlib import Path

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import cid_of, head, minimal_object, publet, author_key, relation

scenarios("../features/conformance/signatures.feature")
scenarios("../features/conformance/object_size.feature")
scenarios("../features/conformance/immutability.feature")

DOMAIN = b"pub/v1/sig"


@given("a well-formed object", target_fixture="payload")
def well_formed_object() -> bytes:
    return minimal_object()


@given("a second object differing only in its created field",
       target_fixture="other_payload")
def other_object() -> bytes:
    return minimal_object(created="2026-09-12T10:00:01Z")


@given(parsers.parse("canonical bytes of exactly {n:d} bytes"),
       target_fixture="payload")
@given(parsers.parse("canonical bytes of {n:d} bytes"), target_fixture="payload")
def sized_payload(n: int) -> bytes:
    """A canonical CBOR byte string occupying exactly `n` bytes on the wire.

    The head length depends on the payload length, and the payload length
    depends on the head, so the head size is solved for rather than assumed.
    """
    for head_len, limit in ((1, 23), (2, 0xFF), (3, 0xFFFF), (5, 0xFFFF_FFFF)):
        body = n - head_len
        if 0 <= body <= limit:
            encoded = head(2, body) + b"\x00" * body
            assert len(encoded) == n, (len(encoded), n)
            return encoded
    raise AssertionError(f"cannot build a {n}-byte object")


@given("a store where a claim has been revised", target_fixture="revision")
def revised_store(tmp_path):
    ka = author_key("K_a")
    original = publet(ka, "2026-09-12T10:00:00Z", "empirical", "the original")
    revision = publet(ka, "2026-09-12T11:00:00Z", "empirical", "the revision")
    rel = relation(ka, "2026-09-12T11:00:01Z", "supersedes",
                   cid_of(revision), cid_of(original))
    for name, data in [("a", original), ("b", revision), ("c", rel)]:
        (tmp_path / f"{name}.cbor").write_bytes(data)
    return {"dir": tmp_path, "original": original, "revision": revision}


@when(parsers.parse('I ask for the message a signature would cover with purpose "{purpose}"'),
      target_fixture="result")
def signing_message(runner, payload: bytes, purpose: str):
    return runner.run("pub-sign", f"--purpose={purpose}", "--message", stdin=payload)


@when(parsers.parse('I ask for the messages for purposes "{a}" and "{b}"'),
      target_fixture="two_results")
def two_messages(runner, payload: bytes, a: str, b: str):
    return [
        runner.run("pub-sign", f"--purpose={p}", "--message", stdin=payload)
        for p in (a, b)
    ]


@when("I ask for a message without naming a purpose", target_fixture="result")
def no_purpose(runner, payload: bytes):
    return runner.run("pub-sign", "--message", stdin=payload)


@when(parsers.parse('I run "{command}" with that object on stdin'),
      target_fixture="result")
def run_with_payload(runner, command: str, payload: bytes):
    return runner.run(command, stdin=payload)


@when(parsers.parse('I run "{command}" on each'), target_fixture="two_results")
def run_on_each(runner, command: str, payload: bytes, other_payload: bytes):
    return [runner.run(command, stdin=p) for p in (payload, other_payload)]


@then("the message begins with the protocol domain string")
def message_domain(result):
    assert result.code == 0, result.stderr
    assert bytes.fromhex(result.stdout.strip()).startswith(DOMAIN)


@then("the purpose appears between separators")
def purpose_separated(result):
    raw = bytes.fromhex(result.stdout.strip())
    assert raw.startswith(DOMAIN + b"\x00endorse\x00"), raw[:24]


@then("the two messages differ")
def messages_differ(two_results):
    first, second = two_results
    assert first.code == second.code == 0
    assert first.out != second.out


@then("neither is a prefix of the other beyond the separator")
def separator_prevents_collision(two_results):
    first, second = (bytes.fromhex(r.stdout.strip()) for r in two_results)
    # Without the trailing separator, "ab" + target and "a" + ("b" + target)
    # would encode identically. With it they cannot.
    assert not first.startswith(second)
    assert not second.startswith(first)


@then("it fails")
def it_fails(result):
    assert result.code != 0, result.stdout


@then("the two identifiers differ")
def identifiers_differ(two_results):
    first, second = two_results
    assert first.code == second.code == 0
    assert first.out != second.out


@then("both the original and the revision are present")
def both_present(runner, revision):
    listed = runner.run("pub-ls", f"--dir={revision['dir']}")
    assert listed.code == 0, listed.stderr
    assert cid_of(revision["original"]) in listed.lines
    assert cid_of(revision["revision"]) in listed.lines


@then("the original still verifies against its own identifier")
def original_intact(runner, revision):
    out = runner.run(
        "pub-verify", f"--cid={cid_of(revision['original'])}",
        stdin=revision["original"],
    )
    assert out.code == 0, out.stderr
