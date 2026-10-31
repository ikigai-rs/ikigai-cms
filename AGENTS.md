# Working in this repository

`ikigai-cms` is an ikigai module: `urn:cms:*` turns personal content (org-mode bookmarks, so far)
into one RDF graph. These rules are for every agent and every person working here. `CLAUDE.md` is
a link to this file.

- **Read [ACCESSIBILITY.md](ACCESSIBILITY.md) before any change to output people read: messages,
  pages, terminal text.** Here that is the Turtle, the note for a dropped heading and the
  endpoint's description.
- Security reports go through the organization's security policy (GitHub private vulnerability
  reporting), never a public issue.
- Every change lands through a pull request with green CI (`.github/workflows/ci.yml`).
- Never invent testing, tool results or conformance evidence, and never weaken a test to make a
  change pass.
- Use American spelling in everything you write: code comments, docs, commit messages and pull
  requests.
