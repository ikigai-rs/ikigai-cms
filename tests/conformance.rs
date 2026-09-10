//! The module recipe as one test: `ikigai-conformance` walks the one endpoint
//! [`ikigai_cms::space`] binds — `bookmarks`, at `urn:cms:bookmarks` — and
//! reports every violation at once.
//!
//! ## The fixture is a bookmarks file by value, because the transreptor reads nothing
//!
//! `urn:cms:bookmarks` opens no file: it transrepts the org text a host pipes into
//! `in` (`source urn:file:bookmarks.org | urn:cms:bookmarks`). So the kernel under
//! test is the module's own space, and the one fixture is a real bookmarks text —
//! the suite's minimal call (`in="x"`, shaped by the `xsd:string` class) is a valid
//! call that yields an EMPTY graph, over which SKOLEM-RDF and VOCABULARY are
//! vacuous (the suite's PENDING #26). Two declarations, stated once:
//!
//! - `pure` — the endpoint reads nothing but `in`: no file, network, clock or
//!   platform read. Its cacheable result rightly carries an empty golden-thread
//!   set: the input is by value, so it is part of the cache key, and the file's
//!   own thread lives on the file's representation upstream of the pipe.
//! - `cacheable` — the result is marked `.cacheable()`. Holding the suite to that
//!   turns a future sub-resolution that silently downgraded the effective expiry
//!   into a red test instead of a ~2000× slowdown (the incident that started in
//!   this graph's consumer, `ikigai-cms-web`).
//!
//! One namespace: Dublin Core ELEMENTS 1.1 (`http://purl.org/dc/elements/1.1/`),
//! the vocabulary a Zotero export speaks and the whole point of the module. It is
//! not in the suite's well-known list (only `dcterms` is), so it is registered
//! here and named in the README.
//!
//! ## What the suite cannot hold and this file pins by hand
//!
//! - **Required is required** ([`in_is_required`]): the minimal call always
//!   supplies `in`, so an `in` the endpoint did without would be invisible to
//!   ARGSPECS (PENDING #49/#99). It was: `unwrap_or("")` served an empty graph for
//!   a call with no input until 0.1.3.
//! - **Declared outputs are the media types served** ([`declared_outputs_are_the_media_types_served`]):
//!   0.1.0 compares nothing that is not an RDF face, and only once declared
//!   (PENDING #11/#31/#79). The Turtle face was served without being an output
//!   until 0.1.3.
//! - **The Turtle face, by hand and in full** ([`the_turtle_face_is_the_bookmark_graph`]):
//!   VOCABULARY sees predicates and `rdf:type` objects only (PENDING #96), so the
//!   subjects — every one a `urn:cms:bookmark:*` skolem — and the objects (all
//!   literals; the URL is a `dc:identifier` literal, never an IRI) are checked here.
//! - **A cached read is a function of its input, timing-free**
//!   ([`a_cached_read_is_a_pure_function_of_its_input`]): the suite's second
//!   resolution IS the cache hit, so it never sees two computations (PENDING #64).
//!   The witness is an independent computation of the same bytes through the pure
//!   function, plus a different `in` served differently while the first stays
//!   cached — the input is the state, so nothing is served stale.
//! - **The fixture id is the description id** ([`the_fixture_id_is_the_description_id`]):
//!   a `Fixture` that matches no description is silently inert (PENDING #57).
//!
//! No opt-outs, NAMES runs (`bookmarks` is kebab-case).

use std::collections::BTreeSet;
use std::sync::Arc;

use ikigai_conformance::{rdf, Fixture, Report, Suite};
use ikigai_core::{
    ArgRef, Capability, Error, Expiry, Iri, Kernel, Representation, Request, Result, Verb,
};

/// The endpoint's description id.
const BOOKMARKS: &str = "bookmarks";
const BOOKMARKS_IRI: &str = "urn:cms:bookmarks";

/// Dublin Core Elements 1.1: the module's one vocabulary, registered with the suite.
const DC: &str = "http://purl.org/dc/elements/1.1/";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The fixture bookmarks file: every shape the parser handles — a `:TAG:` drawer,
/// a `:TAGS:` drawer, a title wrapped across lines, a URL that is not a valid
/// IRI, a link without a title, and prose that is not a bookmark.
const ORG: &str = "\
#+TITLE: Bookmarks

* Bookmarks
some prose about bookmarks
** [[https://webassembly.org][WebAssembly]]
   :PROPERTIES:
   :TAG: webassembly wasm
   :END:
** [[http://x.example/nasa][NASA -
   Aquarius Yields Map]]
   :PROPERTIES:
   :TAGS: nasa science
   :END:
** [[https://ex.com/a%qi][Not an IRI]]
   :TAG: shared
** [[https://ex.com/bare]]
   :TAG: shared
";

/// The bookmark a second `in` adds, to prove a different input is a different result.
const EDIT: &str = "** [[https://ex.com/new][Board game night]]\n   :TAG: games\n";

fn kernel() -> Kernel {
    Kernel::new(Arc::new(ikigai_cms::space()))
}

/// The suite, configured for this module: a real bookmarks text for `in` and the
/// Dublin Core namespace the faces speak.
fn suite() -> Suite {
    Suite::new()
        .fixture(Fixture::new(BOOKMARKS, Verb::Source).arg("in", ORG))
        .namespace(DC)
}

fn source(org: Option<&str>) -> Request {
    let request = Request::new(
        Verb::Source,
        Iri::parse(BOOKMARKS_IRI).expect("a valid IRI"),
    );
    match org {
        Some(org) => request.with_arg("in", ArgRef::Inline(org.as_bytes().to_vec())),
        None => request,
    }
}

fn issue(kernel: &Kernel, request: Request, capability: &Capability) -> Result<Representation> {
    futures::executor::block_on(kernel.issue(request, capability))
}

fn resolve(kernel: &Kernel, request: Request) -> Representation {
    issue(kernel, request, &Capability::root()).unwrap_or_else(|e| panic!("resolution failed: {e}"))
}

fn text(repr: &Representation) -> String {
    String::from_utf8(repr.bytes.clone()).expect("UTF-8")
}

/// The walk saw exactly the one endpoint, one action, and skipped nothing. A
/// second endpoint bound by `space()` without a line here would be held to a
/// weaker standard.
fn assert_shape(report: &Report) {
    assert_eq!(report.endpoints, 1, "bookmarks: {report}");
    assert_eq!(report.actions, 1, "one Source: {report}");
    assert_eq!(
        report.checks.skipped().count(),
        0,
        "every check runs: {report}"
    );
}

#[test]
fn conforms() {
    let kernel = kernel();
    let report = suite()
        .pure(BOOKMARKS)
        .cacheable(BOOKMARKS)
        .run_blocking(&kernel);
    // Printed even when clean (`--nocapture`): the report is the record.
    eprintln!("{report}");
    assert!(report.is_clean(), "{report}");
    assert_shape(&report);
}

/// `Fixture::new(id, …)` is looked up by description id; an id that matches no
/// description is silently unused.
#[test]
fn the_fixture_id_is_the_description_id() {
    let description = kernel()
        .describe(&Iri::parse(BOOKMARKS_IRI).expect("a valid IRI"))
        .expect("urn:cms:bookmarks describes itself");
    assert_eq!(description.id, BOOKMARKS);
    let spec = description
        .action_specs()
        .into_iter()
        .find(|a| a.verb == Verb::Source)
        .expect("Source is declared");
    let input = spec
        .inputs
        .iter()
        .find(|i| i.name == "in")
        .expect("`in` is declared");
    assert!(input.required, "`in` is required");
    assert_eq!(input.class.as_deref(), Some(XSD_STRING));
    assert!(
        spec.requires.is_empty(),
        "an ungated transreptor: it handles only what it is handed"
    );
}

/// Required means required: a call with no `in` is a typed `MissingArgument`
/// naming the input, never an empty graph. An EMPTY `in` is an empty graph — a
/// bookmarks file with nothing in it is a legitimate input.
#[test]
fn in_is_required() {
    let kernel = kernel();
    match issue(&kernel, source(None), &Capability::root()) {
        Err(Error::MissingArgument(name)) => assert_eq!(name, "in"),
        other => panic!("expected MissingArgument(\"in\"), got {other:?}"),
    }
    let empty = resolve(&kernel, source(Some("")));
    let triples = rdf::parse(&empty.repr_type.media_type, &empty.bytes)
        .unwrap_or_else(|e| panic!("an empty bookmarks file is an empty graph: {e}"));
    assert!(triples.is_empty(), "{}", text(&empty));
}

/// What `ikigai-conformance` 0.1.0 does not check: with `as` omitted (there is
/// no `as` — one face) the served bare media type is a declared output, and the
/// one declared output is the one served.
#[test]
fn declared_outputs_are_the_media_types_served() {
    let kernel = kernel();
    let description = kernel
        .describe(&Iri::parse(BOOKMARKS_IRI).expect("a valid IRI"))
        .expect("urn:cms:bookmarks describes itself");
    let spec = description
        .action_specs()
        .into_iter()
        .find(|a| a.verb == Verb::Source)
        .expect("Source is declared");
    let declared: BTreeSet<String> = spec
        .outputs
        .iter()
        .map(|o| rdf::bare_media_type(o))
        .collect();
    assert_eq!(
        declared,
        BTreeSet::from(["text/turtle".to_string()]),
        "one face, declared"
    );
    assert!(
        !spec.inputs.iter().any(|i| i.name == "as"),
        "no `as`: one face needs no selector"
    );
    let served = resolve(&kernel, source(Some(ORG)));
    assert_eq!(
        rdf::bare_media_type(&served.repr_type.media_type),
        "text/turtle"
    );
}

/// The Turtle face, by hand and in full: parses, skolemized under
/// `urn:cms:bookmark:*` (every subject; no blank node), every predicate a Dublin
/// Core element, every object a literal (the URL rides as `dc:identifier`, so a
/// URL that is not an IRI never becomes one), and every bookmark the fixture
/// carries is on the graph. The suite's SKOLEM-RDF and VOCABULARY hold the
/// predicates over the fixture; the subjects and objects are checked here.
#[test]
fn the_turtle_face_is_the_bookmark_graph() {
    let repr = resolve(&kernel(), source(Some(ORG)));
    let ttl = text(&repr);
    let triples = rdf::parse(&repr.repr_type.media_type, &repr.bytes)
        .unwrap_or_else(|e| panic!("the Turtle face parses: {e}\n{ttl}"));
    // 4 bookmarks × (identifier + title) + 6 tags.
    assert_eq!(triples.len(), 14, "a real graph, not an empty one:\n{ttl}");
    assert!(rdf::blank_nodes(&triples).is_empty(), "skolemized:\n{ttl}");

    let mut subjects = BTreeSet::new();
    for triple in &triples {
        let subject = triple.subject.to_string();
        assert!(
            subject.starts_with("<urn:cms:bookmark:") && subject.ends_with('>'),
            "every subject is a skolem bookmark IRI: {subject}"
        );
        assert_eq!(
            subject.len(),
            "<urn:cms:bookmark:>".len() + 16,
            "sixteen hex digits, the FNV-1a of the URL: {subject}"
        );
        subjects.insert(subject);
        let predicate = triple.predicate.as_str();
        assert!(
            predicate.starts_with(DC),
            "the Dublin Core elements only: {predicate}"
        );
        let object = triple.object.to_string();
        assert!(
            object.starts_with('"'),
            "every object is a literal (a URL is never an IRI here): {object}"
        );
    }
    assert_eq!(subjects.len(), 4, "one subject per bookmark:\n{ttl}");
    for term in rdf::terms(&triples) {
        assert!(
            rdf::is_defined(&term, &[DC.to_string()]),
            "`{term}` is nobody's"
        );
    }
    let predicates: BTreeSet<&str> = triples.iter().map(|t| t.predicate.as_str()).collect();
    assert_eq!(
        predicates,
        BTreeSet::from([
            "http://purl.org/dc/elements/1.1/identifier",
            "http://purl.org/dc/elements/1.1/subject",
            "http://purl.org/dc/elements/1.1/title",
        ])
    );

    // The bookmarks, as the reading room reads them back.
    assert!(
        ttl.contains("dc:identifier \"https://webassembly.org\""),
        "{ttl}"
    );
    assert!(ttl.contains("dc:subject \"wasm\""), "{ttl}");
    assert!(
        ttl.contains("dc:title \"NASA - Aquarius Yields Map\""),
        "a wrapped title is stitched:\n{ttl}"
    );
    assert!(
        ttl.contains("dc:subject \"science\""),
        "a :TAGS: drawer:\n{ttl}"
    );
    assert!(
        ttl.contains("dc:identifier \"https://ex.com/a%qi\"")
            && !ttl.contains("<https://ex.com/a%qi>"),
        "a malformed URL is carried verbatim, never as an IRI:\n{ttl}"
    );
    assert!(
        ttl.contains("dc:title \"https://ex.com/bare\""),
        "a link without a title is titled by its URL:\n{ttl}"
    );
    assert_eq!(
        ttl.matches("dc:subject \"shared\"").count(),
        2,
        "the tag axis joins entries:\n{ttl}"
    );
    assert!(!ttl.contains("some prose"), "{ttl}");
}

/// The cache contract, timing-free, with the second computation the suite lacks:
/// the result is `Expiry::Never` with an EMPTY thread set (the input is by value,
/// so it is the cache key — nothing external to cut), the second read is a hit
/// with the same bytes, those bytes are what an independent computation through
/// the pure function produces, and a different `in` is a different result while
/// the first stays cached. A future sub-resolution that downgraded the expiry
/// fails the first assertion here and the `cacheable` declaration in [`conforms`].
#[test]
fn a_cached_read_is_a_pure_function_of_its_input() {
    let kernel = kernel();

    let first = resolve(&kernel, source(Some(ORG)));
    assert_eq!(
        first.expiry,
        Expiry::Never,
        "a pure function: cached outright"
    );
    assert!(
        first.threads().is_empty(),
        "no golden thread: the input IS the state, and it is the cache key"
    );
    assert!(kernel.is_cached(&source(Some(ORG)), &Capability::root()));

    let second = resolve(&kernel, source(Some(ORG)));
    assert_eq!(second.bytes, first.bytes, "the hit serves the same bytes");
    assert_eq!(
        first.bytes,
        ikigai_cms::bookmarks_to_turtle(ORG).into_bytes(),
        "the cached bytes are what a fresh computation of the same input produces"
    );

    // A different input is a different request identity: computed, not served
    // from the first's cache — and the first stays cached, unchanged.
    let edited = format!("{ORG}{EDIT}");
    let third = resolve(&kernel, source(Some(&edited)));
    assert_ne!(third.bytes, first.bytes);
    assert!(text(&third).contains("dc:title \"Board game night\""));
    assert!(!text(&first).contains("Board game night"));
    assert!(kernel.is_cached(&source(Some(ORG)), &Capability::root()));
    assert!(kernel.is_cached(&source(Some(&edited)), &Capability::root()));
    assert_eq!(
        resolve(&kernel, source(Some(ORG))).bytes,
        first.bytes,
        "the first input still serves the first graph"
    );

    // Ungated: a capability holding no grants is served too (nothing is
    // declared, nothing is enforced — the suite's ENFORCED holds the pair).
    let none = Capability::scoped(Vec::<String>::new());
    let ungated = issue(&kernel, source(Some(ORG)), &none)
        .unwrap_or_else(|e| panic!("an ungated transreptor: {e}"));
    assert_eq!(ungated.bytes, first.bytes);
}
