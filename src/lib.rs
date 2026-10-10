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
//! Every heading is a record boundary (a drawer belongs to the heading it sits under);
//! a heading may carry org's default TODO keyword and a priority cookie before its
//! link; a title wrapped across lines is stitched, but never across another heading;
//! and the URL is org's link target with org's escapes removed. See
//! [`bookmarks_to_turtle`].
//!
//! Pure + wasm-clean: it transrepts piped text and never touches the filesystem — a
//! host pipes the file through the kernel
//! (`source urn:file:bookmarks.org | urn:cms:bookmarks`). `in` is required: a call
//! without it is a `MissingArgument`, never an empty graph (an empty `in` IS an empty
//! graph — a bookmarks file with nothing in it). The result is `.cacheable()` and
//! carries no golden thread but its own name's BY DESIGN (core 0.1.73 hangs every
//! cacheable answer on its own name, a thread nothing cuts here): the input arrives by
//! value, so it is part of the cache key, and the file's own thread lives on the file's
//! representation upstream of the pipe — cutting it recomputes the file, which is a new
//! `in`, which is a new key.
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

/// The name [`space`] claims: `urn:iki:space:cms`.
pub const SPACE_ID: &str = "urn:iki:space:cms";

/// The `urn:cms:*` space. Grows as sources are added (Zotero, notes, `oa:` …).
///
/// Configuration-free (no parameters, nothing read while building it), so it names
/// itself [`SPACE_ID`]: every call holds the same doors. The name goes on LAST, since
/// a later `bind` drops it.
pub fn space() -> ikigai_core::EndpointSpace {
    ikigai_core::EndpointSpace::new()
        .bind(Exact::new("urn:cms:bookmarks"), bookmarks())
        .named(ikigai_core::space_iri("cms"))
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
///
/// **Every heading is a record boundary**, link or not: it closes the current bookmark,
/// so a `:TAGS:` drawer belongs to the heading it sits under, and a drawer under a
/// section heading (`* Private notes`) attaches to nothing. A heading whose `[[` never
/// closes is not a link in org, so it is not a bookmark: it is dropped, and the output
/// names its line in a Turtle comment, so the loss is visible without being invented
/// into a resource.
///
/// ```
/// let ttl = ikigai_cms::bookmarks_to_turtle(
///     "** [[https://a.example][A]]\n   :TAGS: web\n* Notes\n   :TAGS: private\n",
/// );
/// assert!(ttl.contains("dc:subject \"web\""));
/// assert!(!ttl.contains("private"), "a section's drawer is not the bookmark's");
///
/// let ttl = ikigai_cms::bookmarks_to_turtle("** [[https://broken][Broken\n** [[https://b][B]]\n");
/// assert!(ttl.contains("# line 1: "), "the dropped heading is named");
/// assert!(ttl.contains("dc:identifier \"https://b\""), "and the next one is kept");
/// ```
pub fn bookmarks_to_turtle(org: &str) -> String {
    // A byte-order mark is not whitespace, so left in place it hides the first heading.
    let org = org.strip_prefix('\u{FEFF}').unwrap_or(org);
    let mut out = format!("@prefix dc: <{DC}> .\n");
    let mut current: Option<Bookmark> = None;
    let lines: Vec<&str> = org.lines().collect();
    let mut next = 0;
    while next < lines.len() {
        let line = lines[next];
        let number = next + 1;
        next += 1;
        let Some(text) = heading(line) else {
            if let (Some(tags), Some(b)) = (tag_line(line), current.as_mut()) {
                b.tags.extend(tags);
            }
            continue;
        };
        emit(&mut out, current.take());
        let Some(link) = link_start(text) else {
            continue; // a section or note heading: a boundary, not a bookmark
        };
        // A Pinboard title can wrap: the heading opens `[[` here but closes `]]` on a
        // later line. Stitch continuation lines on (newline-joined, so the URL and the
        // title can each join their own way) — but never across another heading, which
        // is a record of its own.
        let mut stitched = link.to_string();
        while parse_link(&stitched).is_none()
            && next < lines.len()
            && heading(lines[next]).is_none()
        {
            stitched.push('\n');
            stitched.push_str(lines[next].trim());
            next += 1;
        }
        match parse_link(&stitched) {
            Some((url, title)) => {
                current = Some(Bookmark {
                    url,
                    title,
                    tags: Vec::new(),
                })
            }
            None => out.push_str(&format!(
                "\n# line {number}: skipped a link heading whose `[[` never closes \
                 before the next heading or the end of the file\n"
            )),
        }
    }
    emit(&mut out, current);
    out
}

/// A heading's text after its stars, or `None` for a line that is not a heading. A
/// heading is stars followed by whitespace, the end of the line, or a link (`**[[…`,
/// which this parser has always accepted); `*bold* prose` is not one. Leading
/// indentation is tolerated, as it always has been for link headings — a record
/// boundary must recognize every heading a bookmark can start on.
fn heading(line: &str) -> Option<&str> {
    let rest = line.trim_start();
    let after = rest.trim_start_matches('*');
    if after.len() == rest.len() {
        return None;
    }
    (after.is_empty() || after.starts_with(char::is_whitespace) || after.starts_with("[["))
        .then(|| after.trim_start())
}

/// Org's default TODO keywords (`org-todo-keywords` is `(sequence "TODO" "DONE")`). A
/// file that configures its own keywords (`#+TODO:`) is not read; those stay title text.
const TODO_KEYWORDS: [&str; 2] = ["TODO", "DONE"];

/// The link that IS a heading's title, from `[[` on: org's heading grammar is `STARS
/// KEYWORD PRIORITY TITLE TAGS`, so an optional TODO keyword and an optional priority
/// cookie (`[#A]`) come off first. `None` when the title is not a link — prose before
/// the `[[` (`Notes on [[…]]`) makes a heading ABOUT a link, not a bookmark.
fn link_start(text: &str) -> Option<&str> {
    let mut rest = text;
    for keyword in TODO_KEYWORDS {
        if let Some(after) = rest.strip_prefix(keyword) {
            if after.starts_with(char::is_whitespace) {
                rest = after.trim_start();
                break;
            }
        }
    }
    if let Some(after) = rest.strip_prefix("[#") {
        let mut chars = after.chars();
        if let (Some(p), Some(']')) = (chars.next(), chars.next()) {
            if p != ']' && chars.as_str().starts_with(char::is_whitespace) {
                rest = chars.as_str().trim_start();
            }
        }
    }
    rest.starts_with("[[").then_some(rest)
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

/// `[[target][title]]` (or `[[target]]`) → `(url, title)`; `None` when the link does
/// not close. Tolerant of trailing text after the closing `]]` (org tags, prose).
///
/// The target is scanned escape-aware — a bracket after an odd run of backslashes is
/// the URL's own, not the link's — then a wrapped target is joined with NO space (a
/// URL has none to restore), then org's escapes come off, and only then are the
/// identifier and its skolem taken. The title collapses its whitespace, so a wrapped
/// title reads as one line ("NASA - \n Aquarius…" → "NASA - Aquarius…").
fn parse_link(text: &str) -> Option<(String, String)> {
    let body = text.strip_prefix("[[")?;
    let bytes = body.as_bytes();
    // Whether the byte at `at` is escaped: an odd run of backslashes precedes it.
    let mut escaped = false;
    let mut at = 0;
    let (target, title) = loop {
        // ASCII bytes never occur inside a multi-byte UTF-8 sequence, so every index
        // this stops at is a char boundary.
        match *bytes.get(at)? {
            b'\\' => {
                escaped = !escaped;
                at += 1;
                continue;
            }
            b']' if !escaped => match bytes.get(at + 1) {
                Some(b']') => break (&body[..at], None),
                Some(b'[') => {
                    let (title, _) = body[at + 2..].split_once("]]")?;
                    break (&body[..at], Some(title));
                }
                _ => {}
            },
            _ => {}
        }
        escaped = false;
        at += 1;
    };
    let joined: String = target.split('\n').map(str::trim).collect();
    let url = org_unescape(joined.trim());
    let title = match title {
        Some(title) => collapse_ws(title),
        None => url.clone(),
    };
    Some((url, title))
}

/// Remove org's link-target escapes (Org manual, "Link Format"; `org-link-unescape`).
/// Org escapes a bracket with one backslash and DOUBLES any backslash run that
/// precedes a bracket or ends the target, so: a run of n backslashes before `[`/`]`
/// or at the end becomes n/2, and a run anywhere else is the URL's own, kept whole.
fn org_unescape(target: &str) -> String {
    let mut out = String::with_capacity(target.len());
    let mut chars = target.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut run = 1;
        while chars.next_if_eq(&'\\').is_some() {
            run += 1;
        }
        let kept = match chars.peek() {
            Some('[' | ']') | None => run / 2,
            Some(_) => run,
        };
        out.push_str(&"\\".repeat(kept));
    }
    out
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
