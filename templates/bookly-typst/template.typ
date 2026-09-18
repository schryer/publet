// Thin jtex wrapper around the @preview/bookly Typst Universe package.
// Like templates/charged-ieee in the docs repo, bookly isn't itself a
// jtex/MyST template -- this file is what adapts it into one.
//
// STATUS: does not build against this document yet. Root cause isolated
// by elimination, not guessed at:
//
//   - The move itself is not the problem: the exact same moved content
//     builds clean under lapreprint-typst (53 pages, zero errors).
//   - front-matter/main-matter/tableofcontents are not the problem:
//     removing them entirely and keeping only `#show: bookly.with(...)`
//     still produces the same 62 errors.
//   - `#show: bookly.with(...)` itself is the problem: removing *that*
//     and leaving only `[-IMPORTS-]`/`[-CONTENT-]` (no bookly styling
//     applied at all) builds with zero errors.
//
// So bookly's base styling -- whatever `bookly.with(...)` sets up before
// any content is shown -- does something to raw/code-block handling that
// this document's content relies on. Every code block in the spec full
// of `{ field: <CID>, ... }`-shaped examples has angle brackets as
// placeholder syntax; Typst normally treats `<...>` as label syntax only
// outside raw blocks, and something in bookly's setup appears to make it
// stop treating those blocks as raw. Confirmed via `typst`'s own errors
// ("unclosed label" at the first `<...>` inside a fenced code block,
// cascading into "unclosed raw text" for everything after) -- not
// inferred from documentation, since bookly's own docs do not cover this
// case; the working assumption is bookly redefines `show raw:` as part
// of its base setup, though the package's own source was not read to
// confirm which specific mechanism does it.
//
// Not attempted yet: reading bookly's own Typst source for a raw/code
// related config option, or finding out whether a newer/older bookly
// version behaves differently. Both are real next steps, not ruled out --
// just not done blindly against an unfamiliar package's internals
// without deciding that is actually the way forward.
//
// bookly is also built for multi-file books -- explicit #part/#include'd
// chapters, front-matter/main-matter/appendix/back-matter as sequential
// show rules -- while this document is one long, flat, heading-numbered
// stream (Section 1, 1.1, ... 14.1.2). The README claims plain Typst
// heading syntax works as chapters but never shows it; that question is
// now moot until the raw-block issue above is resolved first.
#import "@preview/bookly:5.1.1": *

#set document(title: [-doc.title-])

// bookly's `author` is a single string, not a list the way lapreprint's
// `authors` is -- taking only the first author is a real simplification,
// not an oversight; joining all of them needs a jtex loop-variable
// pattern ("is this the last iteration") neither existing wrapper in
// this repo uses, so it is left for whichever multi-author document
// actually needs it rather than guessed at here.
#show: bookly.with(
  author: "[-doc.authors[0].name-]",
[# if options.lang #]
  lang: "[-options.lang-]",
[# endif #]
)

#show: front-matter

[# if parts.abstract #]
= Abstract
[-parts.abstract-]
[# endif #]

#tableofcontents

#show: main-matter

[-IMPORTS-]

[-CONTENT-]
