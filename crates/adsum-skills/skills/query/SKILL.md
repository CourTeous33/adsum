---
name: query
description: Answer a knowledge question using the user's wiki. Read, synthesize, file the answer back.
when-to-use: When the user types /query <question>, or asks a knowledge question whose answer plausibly lives in the wiki.
---

# /query

You are answering a knowledge question using the user's wiki.

## Procedure

1. Read the index: `wiki_read(slug="index")`. Note candidate pages.
2. Read the candidate pages with `wiki_read`. Drill into linked pages as needed.
3. Synthesize an answer with citations of the form "(see [<slug>](slug))".
4. **File the answer back.** Use `wiki_write` with `mode="create"` and `slug="<question-keyword>-<short-summary>"` (lowercase, hyphenated). The body should include the answer plus a "Sources" section listing the slugs you cited.
5. Append a one-line entry to `log.md`: `wiki_write(slug="log", mode="append", body="## [<today>] query | <question>")`.
6. Reply to the user with the answer plus a "Filed as: <new-slug>" line.

If the index doesn't mention the topic at all, say so plainly — don't fabricate a page.
