// Thin jtex wrapper around the @preview/classicthesis Typst Universe
// package. Like templates/charged-ieee in the docs repo, classicthesis
// isn't itself a jtex/MyST template -- this file is what adapts it
// into one.
//
// Checked against its real source
// (github.com/adwiteeymauriya/classicthesis-typst), not assumed from
// its Typst Universe page. Two things worth knowing about its shape:
//
//   - Its raw-block show rules (`show raw.where(block: ...)` in
//     lib.typ) only restyle already-parsed raw content -- font,
//     background, box -- the same shape as lapreprint's and
//     charged-ieee's own raw styling. They don't reparse or
//     reinterpret the raw text itself, so this document's
//     angle-bracket-heavy code-fence examples (`<CID>`, `<BCP47 tag>`,
//     etc.) render as literal text rather than tripping Typst's label
//     syntax. Confirmed: 55 pages, zero errors.
//   - Chapters are plain `= Heading` at the top level (see its own
//     template/main.typ example) -- no #part/#include multi-file
//     structure required, matching this document's existing flat,
//     heading-numbered shape with no restructuring needed.
#import "@preview/classicthesis:0.1.0": *

#show: classicthesis.with(
  title: "[-doc.title-]",
  author: "[-doc.authors[0].name-]",
[# if doc.subtitle #]
  subtitle: "[-doc.subtitle-]",
[# endif #]
  // doc.date is a structured MyST value, not a plain string -- naive
  // interpolation produced a literal "[object Object]" on the title
  // page. Left unset so classicthesis falls back to its own default
  // (the current year) rather than guessing at that object's shape.
[# if options.lang #]
  lang: "[-options.lang-]",
[# endif #]
[# if parts.dedication #]
  dedication: [
[-parts.dedication-]
  ],
[# endif #]
[# if parts.abstract #]
  abstract: [
[-parts.abstract-]
  ],
[# endif #]
)

[-IMPORTS-]

[-CONTENT-]
