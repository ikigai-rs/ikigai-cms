//! Semantic-CMS transreptors for ikigai (`urn:cms:*`): turn personal content into
//! one RDF graph where everything is a tagged, linkable, queryable resource.
//!
//! First endpoint: **`urn:cms:bookmarks`** reads an org-mode bookmarks file — level
//! headings of the form `[[url][title]]` with an optional `:TAG:` drawer — and emits
//! Turtle: each bookmark is a resource keyed by its URL, carrying `dc:title` and a
//! `dc:subject` per tag. Using `dc:subject` is deliberate: a Zotero export tags its
//! library items with `dc:subject` too, so bookmarks and books land in ONE tag space
//! with no reconciliation — `?x dc:subject "wasm"` returns both.
//!
//! Pure + wasm-clean: it transrepts piped text and never touches the filesystem — a
//! host pipes the file through the kernel
//! (`source urn:file:bookmarks.org | urn:cms:bookmarks`).

use ikigai_core::{
    ArgSpec, Description, Exact, FnEndpoint, Invocation, ReprType, Representation, Result, Verb,
};

/// The Dublin Core Elements namespace — the vocabulary a Zotero export speaks, so
/// titles (`dc:title`) and tags (`dc:subject`) unify across bookmarks and books.
const DC: &str = "http://purl.org/dc/elements/1.1/";

/// The `urn:cms:*` space. Grows as sources are added (Zotero, notes, `oa:` …).
pub fn space() -> ikigai_core::EndpointSpace {
    ikigai_core::EndpointSpace::new().bind(Exact::new("urn:cms:bookmarks"), bookmarks())
}

fn bookmarks() -> FnEndpoint {
    FnEndpoint::new("bookmarks", bookmarks_impl).with_description(
        Description::new("bookmarks")
            .summary(
                "Transrept an org-mode bookmarks file into RDF/Turtle: each `[[url][title]]` \
                 heading (with an optional `:TAG:` drawer) becomes a resource keyed by its URL, \
                 with dc:title and a dc:subject per tag — the same tag axis a Zotero export uses.",
            )
            .verb(Verb::Source)
            .input(ArgSpec::new("in").summary("the org bookmarks text (piped)")),
    )
}

fn bookmarks_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let org = inv.inline_str("in").unwrap_or("");
    Ok(Representation::new(
        ReprType::new("text/turtle").with_param("charset", "utf-8"),
        bookmarks_to_turtle(org).into_bytes(),
    )
    .cacheable())
}

/// A parsed bookmark: its URL (the resource's identity), title, and tags.
struct Bookmark {
    url: String,
    title: String,
    tags: Vec<String>,
}

/// Transrept org bookmarks → Turtle. Pure — the unit of reuse and test.
pub fn bookmarks_to_turtle(org: &str) -> String {
    let mut out = format!("@prefix dc: <{DC}> .\n");
    let mut current: Option<Bookmark> = None;
    for line in org.lines() {
        if let Some((url, title)) = heading_link(line) {
            emit(&mut out, current.take());
            current = Some(Bookmark {
                url,
                title,
                tags: Vec::new(),
            });
        } else if let Some(tags) = tag_line(line) {
            if let Some(b) = current.as_mut() {
                b.tags.extend(tags);
            }
        }
    }
    emit(&mut out, current);
    out
}

/// Write one bookmark's triples (skolem-free: the URL *is* the subject IRI).
fn emit(out: &mut String, bookmark: Option<Bookmark>) {
    let Some(b) = bookmark else { return };
    out.push_str(&format!(
        "\n<{}> dc:title {}",
        iri_ref(&b.url),
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
        Some((url, title)) => Some((url.trim().to_string(), title.trim().to_string())),
        None => Some((inner.trim().to_string(), inner.trim().to_string())),
    }
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

/// Percent-encode the characters an `<…>` Turtle IRIREF forbids (space and
/// `<>"{}|^\`\\`), leaving an otherwise-verbatim URL.
fn iri_ref(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push_str("%22"),
            '{' => out.push_str("%7B"),
            '}' => out.push_str("%7D"),
            '|' => out.push_str("%7C"),
            '^' => out.push_str("%5E"),
            '`' => out.push_str("%60"),
            '\\' => out.push_str("%5C"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bookmark_becomes_a_dc_resource_keyed_by_its_url() {
        let org = "* Bookmarks\n\
                   ** [[https://webassembly][WebAssembly]]\n\
                   \x20  :PROPERTIES:\n\
                   \x20  :TAG: webassembly wasm\n\
                   \x20  :END:\n";
        let ttl = bookmarks_to_turtle(org);
        assert!(
            ttl.contains("<https://webassembly> dc:title \"WebAssembly\""),
            "{ttl}"
        );
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
        assert!(bookmarks_to_turtle("** [[https://x]]\n")
            .contains("<https://x> dc:title \"https://x\""));
        let ttl = bookmarks_to_turtle("** [[https://y][He said \"hi\"]]\n");
        assert!(ttl.contains("dc:title \"He said \\\"hi\\\"\""), "{ttl}");
    }

    #[test]
    fn non_bookmark_lines_are_ignored() {
        let ttl = bookmarks_to_turtle("* Bookmarks\nsome prose\n* Another Section\n");
        assert!(!ttl.contains("dc:title"), "{ttl}");
    }

    #[test]
    fn a_url_with_a_space_is_iri_escaped() {
        let ttl = bookmarks_to_turtle("** [[https://ex.com/a b][A B]]\n");
        assert!(ttl.contains("<https://ex.com/a%20b>"), "{ttl}");
    }
}
