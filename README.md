# ikigai-cms

Semantic-CMS transreptors for [ikigai](https://github.com/ikigai-rs) (`urn:cms:*`):
turn personal content — org-mode bookmarks, notes, library metadata — into **one
RDF graph** where everything is a tagged, linkable, queryable resource. Views are
SPARQL queries; a blog is a face over one.

The design principle: **don't invent a vocabulary — conform to the one your data
already speaks.** A Zotero export is `bib:`/`dc:`/`foaf:` RDF and tags items with
`dc:subject`. So every source here emits `dc:subject` too, and the tag space
unifies with zero reconciliation:

```sparql
SELECT ?x WHERE { ?x <http://purl.org/dc/elements/1.1/subject> "wasm" }
# returns a book, a bookmark, and a note in one result
```

## Endpoints

`ikigai_cms::space()` is configuration-free, so it names itself `urn:iki:space:cms`
(`ikigai_cms::SPACE_ID`).

- **`urn:cms:bookmarks`** — transrept an org-mode bookmarks file into Turtle. Each
  `[[url][title]]` heading (with an optional `:TAG:` drawer) becomes a resource
  keyed by its URL, with `dc:title` and a `dc:subject` per tag. Pure and
  wasm-clean — it transrepts piped text and never touches the filesystem; a host
  pipes the file through the kernel:

  ```
  source urn:file:bookmarks.org | urn:cms:bookmarks as=text/turtle
  ```

  `in` is required (a call without it is a `MissingArgument`; an empty file is an
  empty graph). The result is cacheable with no golden thread but its own name's by
  design (core 0.1.73 hangs every cacheable answer on its own name, a thread nothing
  cuts here): the input arrives by value and is the cache key, and the file's thread
  lives on the file's representation upstream of the pipe.

## 0.2.0 (2026-10-09)

`ikigai_cms::space()` names itself `urn:iki:space:cms` (`ikigai_cms::SPACE_ID`; ledger
[#987](http://localhost:1060/l/default/item/987)). The name changes what a host sees in
`answered_by`, `urn:kernel:topology` and cache partitioning, so this is a **minor** bump: a
host adopts it deliberately. Requires `ikigai-core` 0.1.89. The endpoint and its output are
unchanged.

## 0.1.4 (2026-10-07)

Parser fixes from the 2026-10-07 audit (ledger [#876](http://localhost:1060/l/default/item/876); `tests/parser_audit.rs`). The
public API is unchanged, so the version call is a **patch: 0.1.4** (not bumped here).

- **Every heading is a record boundary.** A heading whose `[[` never closes no longer
  swallows the bookmark after it (the wrapped-title stitcher stops at any heading), and
  a `:TAGS:` drawer under a non-link heading (`* Private notes`) no longer lands on the
  bookmark before it. The unclosed heading itself is not a link in org, so it is
  dropped, and the output names its line in a Turtle comment
  (`# line N: skipped a link heading …`).
- A heading may carry org's default TODO keyword (`TODO`, `DONE`) and a priority cookie
  (`[#A]`) before its link; it was silently dropped. Prose before the link
  (`Notes on [[…]]`) is still a heading about a link, not a bookmark.
- A URL wrapped across lines is joined with no inserted space (titles still collapse
  their whitespace).
- A leading byte-order mark no longer hides the first heading.
- Org's link-target escapes (`\[`, `\]`, and a doubled backslash run before a bracket
  or at the end) are removed before the identifier and its skolem are computed, so an
  escaped URL gets the same `urn:cms:bookmark:*` subject as the plain URL. That changes
  the skolem of any bookmark whose target was escaped; the live library has none.

What a consumer sees: on the live library (4,301 bookmarks, 183 wrapped titles) the
output is byte-identical before and after. The changes surface only on files with the
malformed or org-native shapes above.

## Conformance

Passes [`ikigai-conformance`](https://github.com/ikigai-rs/ikigai-conformance)
(`tests/conformance.rs`): the endpoint is declared `pure` and `cacheable`, so a
dependency that silently downgraded its effective expiry is a red test. The one
vocabulary the faces speak is Dublin Core Elements 1.1
(`http://purl.org/dc/elements/1.1/`, registered with the suite): `dc:identifier`,
`dc:title`, `dc:subject`. Subjects are `urn:cms:bookmark:{fnv1a-of-url}` skolems;
there is no other `urn:cms:` term and no `ik:` term on the graph.

## Roadmap

Zotero (`My Library.rdf` is already RDF — union it in), notes (`#+TAGS:` → a SKOS
scheme; `dc:subject` on headings; URLs-in-bodies → bookmarks), Web-Annotation
(`oa:`) for highlights/notes on any resource, link-checking (scheduler +
`urn:httpHead` + golden threads), and a WebGPU reading room where a *view is a
query* and *types are renderers*.

## License

MIT OR Apache-2.0.
