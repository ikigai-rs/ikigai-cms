//! The 2026-10-07 audit of the org parser (ledger #876), as tests: two auditors (Claude
//! and Hermes) reproduced six defects against 35efe91, and every test here except the
//! controls at the bottom failed there because of the defect it names.
//!
//! Root cause A: a heading was not a record boundary. Only a LINK heading closed the
//! current bookmark, and the wrapped-title stitcher read past any heading until it
//! found a `]]`. The minors: a TODO-keyword heading was dropped, a wrapped URL gained a
//! space, a leading BOM hid the first heading, and org's link escapes were kept.
//!
//! Assertions read the PARSED graph (the suite's own Turtle parser), so "which
//! bookmark carries this tag" is a question about subjects, not about byte order.

use ikigai_cms::bookmarks_to_turtle;
use ikigai_conformance::rdf;

const DC: &str = "http://purl.org/dc/elements/1.1/";

/// `(subject, predicate, object)` with the IRIs bare and literals as their lexical form.
fn triples(ttl: &str) -> Vec<(String, String, String)> {
    rdf::parse("text/turtle", ttl.as_bytes())
        .unwrap_or_else(|e| panic!("the output must parse: {e}\n{ttl}"))
        .iter()
        .map(|t| {
            let s = t.subject.to_string();
            let p = t.predicate.to_string();
            let o = t.object.to_string();
            (
                s.trim_matches(['<', '>']).to_string(),
                p.trim_matches(['<', '>']).to_string(),
                lexical(&o),
            )
        })
        .collect()
}

/// A literal's lexical form from its N-Triples display (`"a\\b"` → `a\b`); the
/// comparisons below involve backslashes, so the display's escapes must come off.
fn lexical(display: &str) -> String {
    let inner = display
        .strip_prefix('"')
        .and_then(|d| d.strip_suffix('"'))
        .unwrap_or(display);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(u @ ('u' | 'U')) => {
                let width = if u == 'u' { 4 } else { 8 };
                let hex: String = chars.by_ref().take(width).collect();
                let code = u32::from_str_radix(&hex, 16).expect("a \\u escape is hex");
                out.push(char::from_u32(code).expect("a \\u escape is a scalar value"));
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// The subject whose `dc:identifier` is `url`, or `None` when no bookmark carries it.
fn subject_of(ttl: &str, url: &str) -> Option<String> {
    triples(ttl)
        .into_iter()
        .find(|(_, p, o)| p == &format!("{DC}identifier") && o == url)
        .map(|(s, _, _)| s)
}

/// Every `dc:{local}` value on the bookmark whose identifier is `url`.
fn values(ttl: &str, url: &str, local: &str) -> Vec<String> {
    let subject = subject_of(ttl, url).unwrap_or_else(|| panic!("no bookmark for {url}:\n{ttl}"));
    let predicate = format!("{DC}{local}");
    triples(ttl)
        .into_iter()
        .filter(|(s, p, _)| s == &subject && p == &predicate)
        .map(|(_, _, o)| o)
        .collect()
}

// ---- A: every heading is a record boundary --------------------------------------------------

/// Claude BUG-1 / Hermes 1. A heading whose `[[` never closed made the stitcher swallow
/// lines up to the next `]]` — the NEXT bookmark's heading — so that bookmark vanished
/// into the broken one's title and its drawer tags landed on the broken one.
#[test]
fn an_unclosed_link_heading_does_not_swallow_the_next_bookmark() {
    let org = "\
** [[https://broken.example/a][Broken title
** [[https://b.example][B]]
   :PROPERTIES:
   :TAGS: kept
   :END:
** [[https://c.example][C]]
";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(values(&ttl, "https://b.example", "title"), ["B"], "{ttl}");
    assert_eq!(
        values(&ttl, "https://b.example", "subject"),
        ["kept"],
        "{ttl}"
    );
    assert_eq!(values(&ttl, "https://c.example", "title"), ["C"], "{ttl}");
    // The unclosed heading is not a link in org, so it is not a bookmark: it is
    // dropped, and the output says where.
    assert_eq!(subject_of(&ttl, "https://broken.example/a"), None, "{ttl}");
    assert!(
        ttl.contains("# line 1: "),
        "the dropped heading is named: {ttl}"
    );
}

/// Hermes 1, as its auditor wrote it: the unclosed heading is indented and follows a
/// good bookmark, and the bookmark after it carries a `:TAG:` drawer.
#[test]
fn an_indented_unclosed_heading_does_not_take_the_next_bookmarks_drawer() {
    let org = "** [[https://a.example][A]]\n\
               \x20* [[https://unclosed.example][TODO never closed\n\
               ** [[https://b.example][B]]\n\
               \x20  :TAG: keep\n";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(values(&ttl, "https://a.example", "title"), ["A"], "{ttl}");
    assert_eq!(values(&ttl, "https://b.example", "title"), ["B"], "{ttl}");
    assert_eq!(
        values(&ttl, "https://b.example", "subject"),
        ["keep"],
        "{ttl}"
    );
    assert!(
        values(&ttl, "https://a.example", "subject").is_empty(),
        "{ttl}"
    );
    assert!(!ttl.contains("TODO never closed"), "{ttl}");
}

/// The stitcher also stops at the end of the file: a last heading that never closes is
/// dropped, and the bookmark before it keeps exactly its own tags.
#[test]
fn an_unclosed_heading_at_the_end_of_the_file_is_dropped_and_named() {
    let org = "\
** [[https://a.example][A]]
   :TAGS: mine
** [[https://never.example][Never
   closes
   :TAGS: orphan
";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(
        values(&ttl, "https://a.example", "subject"),
        ["mine"],
        "{ttl}"
    );
    assert_eq!(subject_of(&ttl, "https://never.example"), None, "{ttl}");
    assert!(!ttl.contains("orphan"), "{ttl}");
    assert!(ttl.contains("# line 3: "), "{ttl}");
}

/// Claude BUG-2. A `:TAGS:` drawer under a NON-link heading attached to the bookmark
/// before it, because only a link heading closed the current bookmark.
#[test]
fn tags_of_a_non_link_heading_do_not_attach_to_the_previous_bookmark() {
    let org = "\
* Bookmarks
** [[https://a.example][A]]
   :PROPERTIES:
   :TAGS: web
   :END:
* Private notes
  :PROPERTIES:
  :TAGS: secret
  :END:
";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(
        values(&ttl, "https://a.example", "subject"),
        ["web"],
        "{ttl}"
    );
    assert!(
        !ttl.contains("secret"),
        "the section's tag landed somewhere: {ttl}"
    );
}

/// Claude BUG-2b. Written against 35efe91, where the TODO heading was skipped, this
/// asserted that `later` appeared nowhere. With the TODO minor fixed the heading IS a
/// bookmark, so the drawer belongs to it: the claim is that `later` is not on A.
#[test]
fn a_todo_headings_tags_belong_to_the_todo_heading() {
    let org = "\
** [[https://a.example][A]]
   :TAGS: web
** TODO [[https://later.example][Read later]]
   :TAGS: later
";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(
        values(&ttl, "https://a.example", "subject"),
        ["web"],
        "{ttl}"
    );
    assert_eq!(
        values(&ttl, "https://later.example", "subject"),
        ["later"],
        "{ttl}"
    );
}

// ---- the minors ------------------------------------------------------------------------------

/// Hermes 3, narrowed. Org's heading grammar is `STARS KEYWORD PRIORITY TITLE TAGS`, so
/// a TODO keyword and a priority cookie are metadata, not title text: the title is still
/// the link. (Hermes's input was `** TODO Read [[…]]`; the prose word `Read` makes the
/// title "Read <link>", which is not a link heading — see the control below.)
#[test]
fn a_todo_keyword_and_a_priority_cookie_before_the_link_are_accepted() {
    for (org, url) in [
        (
            "** TODO [[https://x.example][Something]]\n",
            "https://x.example",
        ),
        ("** DONE [[https://d.example][Done]]\n", "https://d.example"),
        ("** [#A] [[https://p.example][Prio]]\n", "https://p.example"),
        (
            "*** TODO [#B] [[https://tp.example][Both]]\n",
            "https://tp.example",
        ),
    ] {
        let ttl = bookmarks_to_turtle(org);
        assert!(subject_of(&ttl, url).is_some(), "{org:?} dropped:\n{ttl}");
    }
    let ttl = bookmarks_to_turtle("** TODO [#A] [[https://x.example][Something]]\n");
    assert_eq!(
        values(&ttl, "https://x.example", "title"),
        ["Something"],
        "{ttl}"
    );
}

/// The other half of the TODO decision: only org's DEFAULT keywords (`TODO`, `DONE`) are
/// recognized, and prose before the link still makes a non-link heading.
#[test]
fn prose_or_an_unknown_keyword_before_the_link_is_not_a_bookmark() {
    for org in [
        "** Notes on [[https://x.example][X]]\n",
        "** WAITING [[https://x.example][X]]\n",
        "** TODO Read [[https://x.example][X]]\n",
    ] {
        let ttl = bookmarks_to_turtle(org);
        assert_eq!(
            subject_of(&ttl, "https://x.example"),
            None,
            "{org:?}:\n{ttl}"
        );
    }
}

/// Hermes 2. A URL wrapped across lines was stitched with a SPACE, so `dc:identifier`
/// was not the URL. The URL joins with nothing; the title still collapses whitespace.
#[test]
fn a_url_wrapped_across_lines_is_joined_without_a_space() {
    let org = "** [[https://example.com/a-very-long-path-part-one\n\
               part-two][A wrapped\n\
               \x20  title]]\n";
    let ttl = bookmarks_to_turtle(org);
    let url = "https://example.com/a-very-long-path-part-onepart-two";
    assert_eq!(values(&ttl, url, "title"), ["A wrapped title"], "{ttl}");

    // A wrapped title-less link: the URL is the title too, joined the same way.
    let ttl = bookmarks_to_turtle("** [[https://example.com/one\n   two]]\n");
    let url = "https://example.com/onetwo";
    assert_eq!(values(&ttl, url, "title"), [url], "{ttl}");
}

/// Claude BUG-3. U+FEFF is not whitespace, so a leading byte-order mark hid the first
/// heading and its bookmark was dropped.
#[test]
fn a_leading_bom_does_not_drop_the_first_bookmark() {
    let org = "\u{FEFF}** [[https://first.example][First]]\n   :TAGS: one\n\
               ** [[https://second.example][Second]]\n";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(
        values(&ttl, "https://first.example", "subject"),
        ["one"],
        "{ttl}"
    );
    assert!(
        subject_of(&ttl, "https://second.example").is_some(),
        "{ttl}"
    );
}

/// Claude BUG-4. Org escapes `[`, `]` and a backslash run before them in a link target
/// (Org manual, "Link Format"). The identifier is the unescaped URL, and the skolem is
/// minted from it: the same subject as the plain URL written without escapes.
#[test]
fn org_escapes_in_a_link_target_are_removed_before_the_identifier_and_skolem() {
    let ttl = bookmarks_to_turtle("** [[https://x.example/?q\\[\\]=1][T]]\n");
    let plain = bookmarks_to_turtle("** [[https://x.example/?q[]=1][T]]\n");
    let url = "https://x.example/?q[]=1";
    assert_eq!(values(&ttl, url, "title"), ["T"], "{ttl}");
    assert_eq!(
        subject_of(&ttl, url),
        subject_of(&plain, url),
        "{ttl}\n{plain}"
    );

    // A URL that ENDS in `]`: escaped, it reads `\]` and then the structural `][`.
    let ttl = bookmarks_to_turtle("** [[https://x.example/a\\]][T]]\n");
    assert_eq!(
        values(&ttl, "https://x.example/a]", "title"),
        ["T"],
        "{ttl}"
    );

    // A literal backslash before a bracket is doubled, then the bracket escaped.
    let ttl = bookmarks_to_turtle("** [[https://x.example/a\\\\\\]b][T]]\n");
    assert!(
        subject_of(&ttl, "https://x.example/a\\]b").is_some(),
        "{ttl}"
    );

    // A trailing backslash is doubled so it cannot escape the closing bracket.
    let ttl = bookmarks_to_turtle("** [[https://x.example/a\\\\][T]]\n");
    assert!(subject_of(&ttl, "https://x.example/a\\").is_some(), "{ttl}");
}

/// Org only escapes backslashes that precede a bracket or end the target, so a
/// backslash anywhere else is the URL's own and is kept verbatim.
#[test]
fn a_backslash_org_did_not_escape_is_kept() {
    let ttl = bookmarks_to_turtle("** [[https://x.example/a\\\\b\\c][T]]\n");
    assert!(
        subject_of(&ttl, "https://x.example/a\\\\b\\c").is_some(),
        "{ttl}"
    );
}

// ---- controls: these passed on 35efe91 and must keep passing ---------------------------------

/// Hermes's control: the case the stitcher exists for — a Pinboard title wrapped across
/// lines, its drawer following.
#[test]
fn control_a_wrapped_title_still_stitches() {
    let org = "** [[http://x][NASA - \n\
               Aquarius Yields Map]]\n\
               \x20  :TAGS: nasa science\n";
    let ttl = bookmarks_to_turtle(org);
    assert_eq!(
        values(&ttl, "http://x", "title"),
        ["NASA - Aquarius Yields Map"],
        "{ttl}"
    );
    assert_eq!(values(&ttl, "http://x", "subject"), ["nasa", "science"]);
}

/// Hermes's control: control characters in a title leave the output parseable.
#[test]
fn control_characters_in_titles_still_parse() {
    for title in ["form\x0cfeed", "back\x08space", "nul\x00byte"] {
        let org = format!("** [[https://x.example][{title}]]\n");
        let bytes = bookmarks_to_turtle(&org).into_bytes();
        assert!(rdf::parse("text/turtle", &bytes).is_ok(), "{title:?}");
    }
}

/// Claude's escaping probe: every awkward character, in a URL, a title and a tag,
/// still yields Turtle that parses to a non-empty graph.
///
/// Changed in porting: the probe wrote a lone `\` straight into the target before `][`,
/// which org reads as an ESCAPED `]` — so that link never closes, in org or here, and
/// is now dropped. The target is written the way org writes it (a backslash run before
/// a bracket doubled), and the identifier is checked to come back as the plain URL.
#[test]
fn control_every_awkward_character_escapes_to_valid_turtle() {
    let nasty = [
        "\u{0}",
        "\u{1}",
        "\u{7f}",
        "\u{fffe}",
        "\u{ffff}",
        "\u{10ffff}",
        "\r",
        "\\",
        "\"\"\"",
        "\u{2028}",
        "\\u0041",
        "'",
    ];
    for n in nasty {
        // `org-link-escape` for a target that ends in `n` (only a trailing run changes).
        let escaped = format!(
            "{n}{}",
            "\\".repeat(n.len() - n.trim_end_matches('\\').len())
        );
        for org in [
            format!("** [[https://x{escaped}][t{n}]]\n :TAGS: a{n}b\n"),
            format!("** [[https://y][{n}]]\n"),
        ] {
            let ttl = bookmarks_to_turtle(&org);
            let parsed = rdf::parse("text/turtle", ttl.as_bytes())
                .unwrap_or_else(|e| panic!("{n:?}: {e}\n{ttl}"));
            assert!(!parsed.is_empty(), "{n:?}\n{ttl}");
        }
        // The target is trimmed (as it always was), so whitespace probes round-trip trimmed.
        let ttl = bookmarks_to_turtle(&format!("** [[https://x{escaped}][t]]\n"));
        let url = format!("https://x{n}");
        assert!(subject_of(&ttl, url.trim()).is_some(), "{n:?}\n{ttl}");
    }
}
