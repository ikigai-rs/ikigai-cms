# Accessibility

What `ikigai-cms` emits that people read, the rules that output follows, how it is tested, and
how to report a barrier. It is a library: it writes Turtle text, and a host or a page decides
how a person sees it. It is not a conformance claim. Adapted from the
[ACCESSIBILITY.md template](https://github.com/mgifford/ACCESSIBILITY.md) (MIT).

| Field | Value |
|---|---|
| Target | WCAG 2.2 Level AA, for any page that presents this output; this repository writes text only (Turtle), no HTML or SVG |
| Status | Evaluation in progress. No conformance claim. |
| Owner | the maintainers of this repository |
| Report a barrier | [a GitHub issue](https://github.com/ikigai-rs/ikigai-cms/issues/new?labels=accessibility) with the `accessibility` label; anything security-sensitive through [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-cms/security/advisories/new) |
| Last reviewed | 2026-11-12 |

## What it emits that people read

- **The bookmark graph** (`src/lib.rs`, `bookmarks_to_turtle`, served by `urn:cms:bookmarks`):
  UTF-8 Turtle (`text/turtle; charset=utf-8`) carrying each bookmark's URL, title and tags as
  the org file wrote them. Titles and tags are the author's own text.
- **A comment for a dropped heading**: a link heading whose `[[` never closes is not a
  bookmark, and the output says so in a Turtle comment, ``# line N: skipped a link heading whose
  `[[` never closes before the next heading or the end of the file``.
- **The endpoint's self-description** (`bookmarks()` in `src/lib.rs`): the summary of
  `urn:cms:bookmarks` and of its `in` argument, which a host shows in its catalog, help or tool
  list.

Not this repository's: the text of a refusal such as a missing `in` (`MissingArgument`, whose
words are `ikigai-core`'s), and any page, terminal or tool that shows the graph. This repository
has no binary, so it writes nothing to a terminal.

## The rules that output follows

- A dropped heading is never silent: the comment names its line number and says why it was
  dropped, in words.
- A title wrapped across lines in the org file is joined into one line, its whitespace
  collapsed, so it reads as the author meant it.
- Titles and tags keep the author's words, with Turtle escaping; wrapped-title whitespace is
  collapsed and tags are split on whitespace.
- No color, no terminal control codes and no markup are written.

## How it is tested

Every named check has a result: Pass, Fail or Blocked, with evidence. A Pass covers only
the stated check, not overall accessibility. Results below were run on 2026-11-12 against
main commit `8aa88c47e3caaab56a60847652887f069b184e53`, before this documentation update,
using `cargo test --workspace`, Cargo 1.99.1 and rustc 1.99.1. CI repeats that command
through `.github/workflows/ci.yml`; it does not run a browser or assistive technology.
The reporting format follows [gonk-starter PR 91](https://github.com/ikigai-rs/ikigai-gonk-starter/pull/91).

| Check | Result | Evidence |
|---|---|---|
| Dropped heading is named by line number | Pass | `cargo test --workspace`: `tests/parser_audit.rs` tests `an_unclosed_link_heading_does_not_swallow_the_next_bookmark` and `an_unclosed_heading_at_the_end_of_the_file_is_dropped_and_named`, plus the `bookmarks_to_turtle` doctest in `src/lib.rs`. Pin `# line 1: ` and `# line 3: `; do not pin the explanatory wording or its presentation by a host. |
| Wrapped title is joined and keeps its tags | Pass | Same command: `src/lib.rs` test `a_title_that_wraps_across_lines_is_stitched_and_keeps_its_tags` and `tests/parser_audit.rs` test `control_a_wrapped_title_still_stitches`. Check fixture text and tags, not how a person hears the title. |
| Missing input is refused | Pass | Same command: `tests/conformance.rs` test `in_is_required` checks `MissingArgument` naming `in`, and accepts empty input as an empty graph. Does not evaluate the host's error wording or announcement. |
| Turtle representation and escaping | Pass | Same command: `tests/conformance.rs` tests `declared_outputs_are_the_media_types_served` and `the_turtle_face_is_the_bookmark_graph`; `tests/parser_audit.rs` tests `control_characters_in_titles_still_parse` and `control_every_awkward_character_escapes_to_valid_turtle`. Check media type, fixture graph and parseability, not all possible author text or accessible rendering. |
| Endpoint description is understandable in a host | Blocked | No host presentation or human comprehension evaluation was run. Automated endpoint tests do not establish that the summary is understandable. |
| Browser presentation | Blocked | No browser run on this output; this library supplies Turtle, with no host page or browser test configured here. |
| Manual keyboard | Blocked | No person performed a keyboard pass in a host showing this output; no interactive host session was available for this review. |
| Automated accessibility checker | Blocked | No axe or other accessibility checker was run on this output; no browser checker is configured here. Rust tests are not accessibility checkers. |
| Screen reader | Blocked | No screen reader was used on this output; no screen reader session was available for this review. |
| Screen magnifier | Blocked | No magnifier was used on this output; no magnifier session was available for this review. |
| Speech input | Blocked | No speech-input software was used on this output; no speech-input session was available for this review. |
| Switch access | Blocked | No switch-access software was used on this output; no switch-access session was available for this review. |
| Braille display | Blocked | No braille display was used on this output; no braille display was available for this review. |

## Known limitations

Recorded by source review on 2026-10-31 and rechecked in `src/lib.rs` on 2026-11-12.
These are limitations of main, not results from assistive technology testing:

- The dropped-heading note is a Turtle comment, so it is lost as soon as the output is parsed or
  converted to another format: a person who sees the graph through a host's page or a query
  never sees it.
- Other headings are left out with no note: a heading with prose before its link, or with a TODO
  keyword the file configures itself (`#+TODO:`), and the tags in a drawer under a heading that
  is not a bookmark.
- Titles and tags are written with no language tag, so a page that presents them cannot tell
  their language from the graph.

## Contributing

Read this file before changing any output listed above: the Turtle, the dropped-heading comment
or the endpoint's description. Keep meaning in words, and name the input a message is about. In
the pull request, give each check a row in a `Check | Result | Evidence` table with Pass,
Fail or Blocked, the test or command and tool version, or why it could not run. Keep unrun
checks as Blocked, including each assistive technology nobody used. A historical run that
CI does not repeat is Blocked with its date, never a current Pass.

## Reporting a barrier

[Open an issue](https://github.com/ikigai-rs/ikigai-cms/issues/new?labels=accessibility) with
the `accessibility` label (the link sets it when GitHub lets you add labels; otherwise a
maintainer adds it): what you were doing, what happened, and any assistive technology you use,
if you are comfortable sharing it. Anything security-sensitive goes through
[private vulnerability reporting](https://github.com/ikigai-rs/ikigai-cms/security/advisories/new).
**We acknowledge a report within 7 days**, and promise nothing further.

## Change history

| Date | Change |
|---|---|
| 2026-10-31 | First version. |
| 2026-11-12 | Record scoped test results on main and explicit Blocked checks for host presentation and assistive technology. |
