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
| Last reviewed | 2026-10-31 |

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
- Titles and tags are written verbatim, with only the escapes Turtle needs, so nothing a person
  wrote is reworded.
- No color, no terminal control codes and no markup are written.

## How it is tested

- `tests/parser_audit.rs`: `an_unclosed_link_heading_does_not_swallow_the_next_bookmark` and
  `an_unclosed_heading_at_the_end_of_the_file_is_dropped_and_named` pin that a dropped heading
  is named by its line (`# line 1: `, `# line 3: `); the doctest on `bookmarks_to_turtle`
  pins the same. The wording after the line number is not pinned.
- `src/lib.rs`'s `a_title_that_wraps_across_lines_is_stitched_and_keeps_its_tags` pins the
  joined title.
- `tests/conformance.rs`'s `in_is_required` pins that a missing `in` is refused as
  `MissingArgument` naming `in`.

**No automated accessibility checker, browser or assistive technology has been used on this
output.**

## Known limitations

Found by reading the source on 2026-10-31, not by testing:

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
or the endpoint's description. Keep meaning in words, and name the input a message is about. Say
in the pull request what was tested, with what, and what was not.

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
