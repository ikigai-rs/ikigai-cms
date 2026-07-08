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

- **`urn:cms:bookmarks`** — transrept an org-mode bookmarks file into Turtle. Each
  `[[url][title]]` heading (with an optional `:TAG:` drawer) becomes a resource
  keyed by its URL, with `dc:title` and a `dc:subject` per tag. Pure and
  wasm-clean — it transrepts piped text and never touches the filesystem; a host
  pipes the file through the kernel:

  ```
  source urn:file:bookmarks.org | urn:cms:bookmarks as=text/turtle
  ```

## Roadmap

Zotero (`My Library.rdf` is already RDF — union it in), notes (`#+TAGS:` → a SKOS
scheme; `dc:subject` on headings; URLs-in-bodies → bookmarks), Web-Annotation
(`oa:`) for highlights/notes on any resource, link-checking (scheduler +
`urn:httpHead` + golden threads), and a WebGPU reading room where a *view is a
query* and *types are renderers*.

## License

MIT OR Apache-2.0.
