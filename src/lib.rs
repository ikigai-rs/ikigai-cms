//! Semantic-CMS transreptors for ikigai (`urn:cms:*`): turn personal content into
//! one RDF graph where everything is a tagged, linkable, queryable resource.
//!
//! First endpoint: **`urn:cms:bookmarks`** reads an org-mode bookmarks file — level
//! headings of the form `[[url][title]]` with an optional `:TAG:` drawer — and emits
//! Turtle: each bookmark is a **skolemized** resource (a stable `urn:cms:bookmark:*`
//! IRI hashed from its URL — real URLs aren't all valid IRIs), carrying its URL as a
//! `dc:identifier` literal, a `dc:title`, and a `dc:subject` per tag. Using
//! `dc:subject` is deliberate: a Zotero export tags its library items with
//! `dc:subject` too, so bookmarks and books land in ONE tag space with no
//! reconciliation — `?x dc:subject "wasm"` returns both.
//!
//! Pure + wasm-clean: it transrepts piped text and never touches the filesystem — a
//! host pipes the file through the kernel
//! (`source urn:file:bookmarks.org | urn:cms:bookmarks`). `in` is required: a call
//! without it is a `MissingArgument`, never an empty graph (an empty `in` IS an empty
//! graph — a bookmarks file with nothing in it). The result is `.cacheable()` with an
//! empty golden-thread set BY DESIGN: the input arrives by value, so it is part of the
//! cache key, and the file's own thread lives on the file's representation upstream of
//! the pipe — cutting it recomputes the file, which is a new `in`, which is a new key.
//! Nothing here can be served stale, and nothing here needs cutting.
//!
//! The module recipe is held by `ikigai-conformance` (`tests/conformance.rs`): the
//! one endpoint is declared `pure` and `cacheable`, so a future dependency that
//! silently downgraded the effective expiry is a red test, not a slow read.

use ikigai_core::{
    ArgSpec, Description, Exact, FnEndpoint, Invocation, ReprType, Representation, Result, Verb,
};

/// The Dublin Core Elements namespace — the vocabulary a Zotero export speaks, so
/// titles (`dc:title`) and tags (`dc:subject`) unify across bookmarks and books.
const DC: &str = "http://purl.org/dc/elements/1.1/";

/// The XSD datatype of `in`: the org text itself, by value.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The `urn:cms:*` space. Grows as sources are added (Zotero, notes, `oa:` …).
pub fn space() -> ikigai_core::EndpointSpace {
    ikigai_core::EndpointSpace::new().bind(Exact::new("urn:cms:bookmarks"), bookmarks())
}

fn bookmarks() -> FnEndpoint {
    FnEndpoint::new("bookmarks", bookmarks_impl).with_description(
        Description::new("bookmarks")
            .summary(
                "Transrept an org-mode bookmarks file into RDF/Turtle: each `[[url][title]]` \
                 heading (with an optional `:TAG:` drawer) becomes a skolemized \
                 `urn:cms:bookmark:*` resource carrying its URL as dc:identifier, a dc:title, \
                 and a dc:subject per tag — the same tag axis a Zotero export uses.",
            )
            .verb(Verb::Source)
            .input(
                ArgSpec::new("in")
                    .summary("the org bookmarks text (piped)")
                    .class(XSD_STRING),
            )
            .output("text/turtle"),
    )
}

fn bookmarks_impl(inv: &Invocation<'_>) -> Result<Representation> {
    // Required means required: absent is `MissingArgument`, not an empty graph.
    let org = inv.inline_str("in")?;
    Ok(Representation::new(
        ReprType::new("text/turtle").with_param("charset", "utf-8"),
        bookmarks_to_turtle(org).into_bytes(),
    )
    .cacheable())
}

/// A parsed bookmark: its URL (carried as `dc:identifier`; the subject IRI is a
/// skolem hashed from it), title, and tags.
struct Bookmark {
    url: String,
    title: String,
    tags: Vec<String>,
}

/// Transrept org bookmarks → Turtle. Pure — the unit of reuse and test.
pub fn bookmarks_to_turtle(org: &str) -> String {
    let mut out = format!("@prefix dc: <{DC}> .\n");
    let mut current: Option<Bookmark> = None;
    let mut lines = org.lines();
    while let Some(line) = lines.next() {
        // A Pinboard title can wrap: the heading opens `[[` here but closes `]]`
        // on a later line. Stitch the continuation lines back on (as spaces) so a
        // wrapped title parses like any other — otherwise the whole bookmark, tags
        // and all, is silently dropped.
        let stitched;
        let heading = if opens_unclosed_link(line) {
            let mut buf = line.to_string();
            for cont in lines.by_ref() {
                buf.push(' ');
                buf.push_str(cont.trim());
                if cont.contains("]]") {
                    break;
                }
            }
            stitched = buf;
            stitched.as_str()
        } else {
            line
        };
        if let Some((url, title)) = heading_link(heading) {
            emit(&mut out, current.take());
            current = Some(Bookmark {
                url,
                title,
                tags: Vec::new(),
            });
        } else if let Some(tags) = tag_line(heading) {
            if let Some(b) = current.as_mut() {
                b.tags.extend(tags);
            }
        }
    }
    emit(&mut out, current);
    out
}

/// A heading line that opens an org link (`* … [[`) but doesn't close it (`]]`)
/// on the same line — the signal to stitch continuation lines (a wrapped title).
fn opens_unclosed_link(line: &str) -> bool {
    let rest = line.trim_start();
    rest.starts_with('*')
        && rest.trim_start_matches('*').trim_start().starts_with("[[")
        && !line.contains("]]")
}

/// Write one bookmark's triples. Skolemized: the subject is a stable minted IRI (a
/// hash of the URL), NOT the raw URL — real bookmark URLs aren't all valid IRIs. The
/// URL rides as a `dc:identifier` literal, so a malformed URL (bare `%`, stray `#`, …)
/// can never leak into an invalid subject IRI.
fn emit(out: &mut String, bookmark: Option<Bookmark>) {
    let Some(b) = bookmark else { return };
    out.push_str(&format!(
        "\n<{}> dc:identifier {} ;\n    dc:title {}",
        skolem(&b.url),
        ttl_str(&b.url),
        ttl_str(&b.title)
    ));
    for tag in &b.tags {
        out.push_str(&format!(" ;\n    dc:subject {}", ttl_str(tag)));
    }
    out.push_str(" .\n");
}

/// A `*…* [[url][title]]` (or `[[url]]`) org heading → `(url, title)`; `None` for
/// any heading that isn't a link, so prose and section headings are ignored.
/// Tolerant of trailing org tags/text after the link (`… ]] :foo:`).
fn heading_link(line: &str) -> Option<(String, String)> {
    let rest = line.trim_start();
    if !rest.starts_with('*') {
        return None;
    }
    let after = rest.trim_start_matches('*').trim_start();
    let after = after.strip_prefix("[[")?;
    let (inner, _) = after.split_once("]]")?;
    match inner.split_once("][") {
        // Collapse the title's internal whitespace so a stitched, wrapped title
        // reads as one clean line ("NASA - \n Aquarius…" → "NASA - Aquarius…").
        Some((url, title)) => Some((url.trim().to_string(), collapse_ws(title))),
        None => Some((inner.trim().to_string(), inner.trim().to_string())),
    }
}

/// Collapse internal whitespace runs (including the newline joins from a stitched,
/// wrapped title) into single spaces, and trim.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A `:TAG:`/`:TAGS:` drawer line (case-insensitive) → its whitespace-separated
/// tags; `None` for any other line.
fn tag_line(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    let lower = t.to_ascii_lowercase();
    let cut = if lower.starts_with(":tag:") {
        5
    } else if lower.starts_with(":tags:") {
        6
    } else {
        return None;
    };
    Some(t[cut..].split_whitespace().map(str::to_string).collect())
}

/// A Turtle string literal with the minimal escapes.
fn ttl_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A stable, opaque, always-valid IRI for a bookmark: FNV-1a over its URL. The URL
/// itself is carried as a `dc:identifier` literal, so a malformed URL can never leak
/// into an invalid subject IRI. Same URL → same subject, so the graph stays diffable.
fn skolem(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("urn:cms:bookmark:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bookmark_becomes_a_skolemized_resource_carrying_its_url() {
        let org = "* Bookmarks\n\
                   ** [[https://webassembly][WebAssembly]]\n\
                   \x20  :PROPERTIES:\n\
                   \x20  :TAG: webassembly wasm\n\
                   \x20  :END:\n";
        let ttl = bookmarks_to_turtle(org);
        // Subject is a stable minted urn; the URL rides as a dc:identifier literal.
        assert!(ttl.contains("<urn:cms:bookmark:"), "{ttl}");
        assert!(
            ttl.contains("dc:identifier \"https://webassembly\""),
            "{ttl}"
        );
        assert!(ttl.contains("dc:title \"WebAssembly\""), "{ttl}");
        assert!(ttl.contains("dc:subject \"webassembly\""), "{ttl}");
        assert!(ttl.contains("dc:subject \"wasm\""), "{ttl}");
    }

    #[test]
    fn tags_share_the_dc_subject_axis_across_entries() {
        // The whole point: `dc:subject` is the join. "shared" appears on both.
        let org = "** [[https://a][A]]\n   :TAG: shared\n\
                   ** [[https://b][B]]\n   :TAG: shared other\n";
        let ttl = bookmarks_to_turtle(org);
        assert_eq!(ttl.matches("dc:subject \"shared\"").count(), 2, "{ttl}");
    }

    #[test]
    fn a_link_without_a_title_uses_the_url_and_a_title_quote_is_escaped() {
        let ttl = bookmarks_to_turtle("** [[https://x]]\n");
        assert!(ttl.contains("dc:identifier \"https://x\""), "{ttl}");
        assert!(ttl.contains("dc:title \"https://x\""), "{ttl}");
        let ttl = bookmarks_to_turtle("** [[https://y][He said \"hi\"]]\n");
        assert!(ttl.contains("dc:title \"He said \\\"hi\\\"\""), "{ttl}");
    }

    #[test]
    fn non_bookmark_lines_are_ignored() {
        let ttl = bookmarks_to_turtle("* Bookmarks\nsome prose\n* Another Section\n");
        assert!(!ttl.contains("dc:title"), "{ttl}");
    }

    #[test]
    fn a_title_that_wraps_across_lines_is_stitched_and_keeps_its_tags() {
        // A Pinboard title split over two lines (the closing `]]` on the second) —
        // the whole bookmark, including the drawer that follows, must survive.
        let org = "** [[http://x][NASA - \n\
                   Aquarius Yields Map]]\n\
                   \x20  :PROPERTIES:\n\
                   \x20  :TAGS: nasa science\n\
                   \x20  :END:\n";
        let ttl = bookmarks_to_turtle(org);
        assert!(ttl.contains("dc:identifier \"http://x\""), "{ttl}");
        assert!(
            ttl.contains("dc:title \"NASA - Aquarius Yields Map\""),
            "{ttl}"
        );
        assert!(ttl.contains("dc:subject \"nasa\""), "{ttl}");
        assert!(ttl.contains("dc:subject \"science\""), "{ttl}");
    }

    #[test]
    fn a_malformed_url_yields_a_valid_subject_and_a_verbatim_identifier() {
        // The reason to skolemize: a bare `%`, a stray `#`, or a space make a URL an
        // invalid IRI. It must never be the subject — the subject is always a urn, and
        // the URL is carried verbatim as a dc:identifier literal.
        for url in [
            "https://ex.com/a%qi",
            "https://ex.com/p#a#b",
            "https://ex.com/a b",
        ] {
            let ttl = bookmarks_to_turtle(&format!("** [[{url}][X]]\n"));
            assert!(ttl.contains("<urn:cms:bookmark:"), "not skolemized: {ttl}");
            assert!(
                !ttl.contains(&format!("<{url}>")),
                "raw URL leaked as IRI: {ttl}"
            );
            assert!(
                ttl.contains(&format!("dc:identifier \"{url}\"")),
                "URL not carried verbatim: {ttl}"
            );
        }
    }

    #[test]
    fn the_same_url_skolemizes_stably() {
        // Same URL → same subject (diffable), regardless of the title.
        let ttl = bookmarks_to_turtle("** [[https://ex.com/x][Anything]]\n");
        assert!(
            ttl.contains(&format!("<{}>", skolem("https://ex.com/x"))),
            "{ttl}"
        );
    }
}
