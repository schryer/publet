#!/usr/bin/env python3
"""Fetch lead sentences for named Wikipedia articles by HTTP range request.

Seed tooling for the self-hosting corpus. Not protocol code: nothing in the
Rust workspace depends on it, and the conformance suite never invokes it.

The point is not to download Wikipedia. The full English dump is 26.8 GB as
one multistream file, and the corpus needs on the order of 300 articles. A
multistream .bz2 is a concatenation of independently decompressible bz2
streams of 100 pages each, and the companion index gives, per article, the
byte offset of the stream holding it:

    offset:pageid:title

So one article costs one HTTP range request for a single stream -- about
1.6 MB -- and the other 26.8 GB is never transferred. The index itself is
285 MB and is fetched once.

Output is one JSON object per line: title, page id, revision id, and the
lead sentence. Revision id is the whole point of recording it: it is what
makes the resulting `attributive` publet verifiable by a third party, who
can re-fetch exactly the revision quoted.

Usage:
    wiki-seed.py --index=PATH --titles=FILE [--out=FILE]
    wiki-seed.py --fetch-index=PATH        # one-time, 285 MB
"""

from __future__ import annotations

import argparse
import bz2
import html
import json
import re
import sys
import urllib.request

BASE = "https://dumps.wikimedia.org/enwiki/latest"
DUMP = f"{BASE}/enwiki-latest-pages-articles-multistream.xml.bz2"
INDEX = f"{BASE}/enwiki-latest-pages-articles-multistream-index.txt.bz2"

MEDIA = ("file:", "image:", "category:")
BOLD = "'" * 3
ITALIC = "'" * 2


# --- wikitext ---------------------------------------------------------------
#
# Regex alone cannot do this. Templates ({{...}}) and media links
# ([[File:...]]) both nest, and a non-greedy match stops at the first inner
# terminator -- which is how a file caption ends up masquerading as the lead
# sentence. Every bracketed construct below is matched by depth counting.

def _spans(text: str, open_t: str, close_t: str):
    """Yield (start, end, inner) for each top-level balanced pair."""
    depth, start, i = 0, None, 0
    while i < len(text):
        if text.startswith(open_t, i):
            if depth == 0:
                start = i
            depth += 1
            i += len(open_t)
        elif text.startswith(close_t, i):
            if depth > 0:
                depth -= 1
                i += len(close_t)
                if depth == 0 and start is not None:
                    yield start, i, text[start + len(open_t):i - len(close_t)]
                    start = None
            else:
                i += len(close_t)
        else:
            i += 1


def _replace(text: str, open_t: str, close_t: str, fn) -> str:
    out, last = [], 0
    for start, end, inner in _spans(text, open_t, close_t):
        out.append(text[last:start])
        out.append(fn(inner))
        last = end
    out.append(text[last:])
    return "".join(out)


def _link(inner: str) -> str:
    if inner.strip().lower().startswith(MEDIA):
        return ""                      # media and category links: drop whole
    return inner.split("|")[-1]        # piped link: keep the display text


def lead_sentence(wikitext: str) -> str:
    """The article's first sentence, or "" for a redirect or disambiguation.

    Order matters as much as method. The dump XML-escapes the wikitext, so
    an HTML comment arrives as &lt;!--...--&gt;; a comment stripper run
    before the entity decode passes it straight through into the result.
    """
    t = html.unescape(wikitext)
    t = re.sub(r"<!--.*?-->", "", t, flags=re.S)
    t = re.sub(r"<ref[^>]*>.*?</ref>|<ref[^>]*/>", "", t, flags=re.S)
    t = _replace(t, "{{", "}}", lambda _: "")     # templates, infoboxes
    t = _replace(t, "{|", "|}", lambda _: "")     # tables
    t = _replace(t, "[[", "]]", _link)            # links, media, categories
    t = re.sub(r"<[^>]+>", "", t)
    t = t.replace(BOLD, "").replace(ITALIC, "")
    t = re.sub(r"[ \t]+", " ", t)
    for line in t.split("\n"):
        line = line.strip()
        if len(line) > 40 and not line.startswith(("*", "|", "=", ":", ";", "#")):
            m = re.search(r"\.(?=\s+[A-Z])", line)
            return (line[: m.start() + 1] if m else line).strip()
    return ""


# --- dump access ------------------------------------------------------------

def fetch_range(url: str, start: int, end: int) -> bytes:
    """Bytes [start, end) over HTTP. The server must honour Range."""
    request = urllib.request.Request(url, headers={
        "Range": f"bytes={start}-{end - 1}",
        "User-Agent": "publet-seed/0.1 (self-hosting corpus; contact repo owner)",
    })
    with urllib.request.urlopen(request) as response:
        if response.status != 206:
            raise RuntimeError(
                f"server ignored Range and returned {response.status}; "
                "refusing to download the whole dump"
            )
        return response.read()


def load_index(path: str, wanted: set[str]) -> dict[str, tuple[int, int, int]]:
    """Map each wanted title to (stream_start, stream_end, page_id).

    The index is sorted by offset, so the end of a stream is the next
    distinct offset. Both are needed: without the end there is no range to
    request, only a guess at how much to pull.
    """
    entries: dict[str, tuple[int, int]] = {}
    offsets: set[int] = set()
    with bz2.open(path, "rt", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            head, _, title = line.rstrip("\n").partition(":")
            page_id, _, title = title.partition(":")
            if not head.isdigit():
                continue
            offsets.add(int(head))
            if title in wanted:
                entries[title] = (int(head), int(page_id))

    ordered = sorted(offsets)
    following = {a: b for a, b in zip(ordered, ordered[1:])}
    return {
        title: (start, following.get(start, start + (1 << 22)), page_id)
        for title, (start, page_id) in entries.items()
    }


def articles_in_stream(start: int, end: int) -> dict[str, tuple[str, str]]:
    """Every page in one stream, as title -> (revision id, wikitext)."""
    raw = fetch_range(DUMP, start, end)
    xml = bz2.BZ2Decompressor().decompress(raw).decode("utf-8", "replace")
    found = {}
    for page in re.findall(r"<page>.*?</page>", xml, re.S):
        title = re.search(r"<title>(.*?)</title>", page, re.S)
        revision = re.search(r"<revision>.*?<id>(\d+)</id>", page, re.S)
        body = re.search(r"<text[^>]*>(.*?)</text>", page, re.S)
        if title and revision and body:
            found[title.group(1)] = (revision.group(1), body.group(1))
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", help="local path to the multistream index")
    parser.add_argument("--titles", help="file of article titles, one per line")
    parser.add_argument("--out", help="output path (default: stdout)")
    parser.add_argument("--fetch-index", help="download the 285 MB index here")
    args = parser.parse_args()

    if args.fetch_index:
        print(f"fetching {INDEX}\n  -> {args.fetch_index} (285 MB)", file=sys.stderr)
        urllib.request.urlretrieve(INDEX, args.fetch_index)
        return 0

    if not args.index or not args.titles:
        parser.error("--index and --titles are required")

    with open(args.titles, encoding="utf-8") as handle:
        wanted = {line.strip() for line in handle if line.strip()}

    located = load_index(args.index, wanted)
    missing = wanted - set(located)
    if missing:
        print(f"not in index: {sorted(missing)}", file=sys.stderr)

    # One request per stream, not per article: articles are often adjacent,
    # and two titles in one stream must not cost two transfers.
    by_stream: dict[tuple[int, int], list[str]] = {}
    for title, (start, end, _) in located.items():
        by_stream.setdefault((start, end), []).append(title)

    out = open(args.out, "w", encoding="utf-8") if args.out else sys.stdout
    written = skipped = 0
    try:
        for (start, end), titles in sorted(by_stream.items()):
            pages = articles_in_stream(start, end)
            for title in titles:
                if title not in pages:
                    print(f"absent from its stream: {title}", file=sys.stderr)
                    continue
                revision, body = pages[title]
                sentence = lead_sentence(body)
                if not sentence:
                    # A redirect or disambiguation page has no lead sentence.
                    skipped += 1
                    continue
                json.dump({
                    "title": title,
                    "page_id": located[title][2],
                    "revision": revision,
                    "lead": sentence,
                    "source": "en.wikipedia.org",
                    "license": "CC BY-SA 4.0",
                }, out, ensure_ascii=False)
                out.write("\n")
                written += 1
    finally:
        if args.out:
            out.close()

    print(f"{written} written, {skipped} without a lead sentence "
          f"(redirects and disambiguation pages), "
          f"{len(by_stream)} range requests", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
