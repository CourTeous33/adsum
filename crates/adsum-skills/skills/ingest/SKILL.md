---
name: ingest
description: Fetch a URL, summarize, create wiki pages for the entities/concepts you find, and update the index.
when-to-use: When the user types /ingest <url>, or pastes a URL with the implied request to add it to their wiki.
---

# /ingest

You are adding new content to the user's wiki from an external source.

## Procedure

1. Fetch: `web_article(url=<url>)`. This runs Mozilla-Readability-style extraction and gives you `{ title, byline, body, ... }` already cleaned of HTML markup. Use `web_fetch` instead only if the URL is a JSON API, RSS feed, or you specifically need the raw page (uncommon for /ingest).
2. Identify the **subject** (one main entity or concept) and any **secondary entities** worth their own pages.
3. For each entity, create a page: `wiki_write(slug="<entity-slug>", mode="create", body="<page>")`. Each page should include:
   - A one-line definition at the top.
   - Sections appropriate to the content (e.g., "Background", "Key claims", "Open questions").
   - A "Source" line at the bottom: `Source: <url>`.
4. Update `index.md` so the new pages are discoverable: `wiki_read(slug="index")`, edit appropriately, `wiki_write(slug="index", mode="overwrite", body=<updated>)`. Group pages by topic.
5. Append a log entry: `wiki_write(slug="log", mode="append", body="## [<today>] ingest | <subject> ([source](<url>))")`.
6. Reply to the user with a summary: subject, list of created slugs, and one sentence on what you skipped.

If the fetch fails, say so plainly and don't write anything.
