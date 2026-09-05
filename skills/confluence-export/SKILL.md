---
name: confluence-export
description: Search Confluence through its REST API, then export or refresh a page, subtree, space, or organisation as Markdown with cme. Use for finding Confluence pages, backups, migrations, local mirrors, or staging source pages in an Obsidian vault.
requires_bin: cme
command: cme
---

# Search and Export Confluence

Search before export when the user does not have a page URL. Export the smallest
requested scope to Markdown. A page, subtree, space, and organisation are progressively
broader scopes.

## 1. Prepare access

Check `cme version`. If it is missing, offer:

```sh
loom add --tool confluence-markdown-exporter --dry-run
```

Show the install command without `--dry-run` and obtain confirmation before running it.

Authentication is interactive. Ask the user to run this in their own terminal; tokens
stay out of chat and command history:

```sh
cme config edit auth.confluence
```

Use `cme <command> --help` as the current option reference. This step is complete when
`cme` is available and the user confirms that authentication is configured.

## 2. Search

When the user gives search terms instead of a URL, run the bundled helper:

```sh
python3 <skill-directory>/scripts/search.py "integration guidelines"
```

Use `--space KEY` to limit the search to one space. Use `--site URL` only when the cme
configuration contains several Confluence instances. The helper reads the credential
file reported by `cme config path`, sends an official CQL content-search request, and
prints page ids, titles, spaces, update times, and URLs as JSON. It never prints the
username or token.

Show the matching titles and URLs. Ask the user which result and scope to export when
more than one reasonable match remains. This step is complete when one source URL is
selected or when the user only requested search results.

## 3. Set the export boundary

Collect the Confluence URL and choose one command:

| Scope                | Command                                 |
| -------------------- | --------------------------------------- |
| Page                 | `cme pages <page-url>`                  |
| Page and descendants | `cme pages-with-descendants <page-url>` |
| Space                | `cme spaces <space-url>`                |
| Organisation         | `cme orgs <base-url>`                   |

For a wiki export, resolve the selected vault and stage under
`<vault>/inbox/confluence/<space>/`. Keep exported source material out of
canonical wiki folders.

Use a dedicated export directory containing no hand-edited files. CME overwrites
exported pages and may remove local files when Confluence pages are deleted or moved.
Confluence permissions are not reproduced on disk, so the destination must be at least
as private as every page in scope. Confirm the destination before every export and
obtain explicit confirmation for subtree, space, or organisation scope. This step is
complete when scope, dedicated destination, and access boundary are explicit.

## 4. Export

For a plain Markdown export, set the destination for the selected scope command:

```sh
CME_EXPORT__OUTPUT_PATH="$DEST" cme pages "$SOURCE_URL"
```

For Obsidian staging, use per-command overrides so the user's persistent settings stay
untouched:

```sh
CME_EXPORT__OUTPUT_PATH="$DEST" \
CME_EXPORT__INCLUDE_DOCUMENT_TITLE=false \
CME_EXPORT__PAGE_BREADCRUMBS=false \
CME_EXPORT__PAGE_HREF=relative \
CME_EXPORT__ATTACHMENT_HREF=relative \
CME_EXPORT__PAGE_PROPERTIES_FORMAT=frontmatter \
CME_EXPORT__CONFLUENCE_URL_IN_FRONTMATTER=tinyui \
CME_EXPORT__PAGE_METADATA_IN_FRONTMATTER=true \
  cme spaces "$SOURCE_URL"
```

Use the command selected in step 1 in place of `pages` or `spaces`. Relative links avoid
ambiguous Obsidian links when pages share a title; use `wiki` links only when the user
accepts that collision risk. Reusing the dedicated destination skips unchanged pages
and cleans up files for deleted or moved pages. This step is complete when `cme` exits
successfully and its summary has been captured. Continue to verification after any
non-zero exit or reported failure, but treat the export as incomplete.

## 5. Verify and hand off

Read the export summary and report:

- scope and destination;
- exported, unchanged, removed, and failed page counts;
- exported, unchanged, removed, and failed attachment counts;
- Markdown and attachment paths;
- any unsupported macros or missing attachments reported by `cme`.

A wiki export is complete when the source files are present under `inbox/confluence/`
and every reported failure is visible. Stop after staging unless the user also asked
for canonical ingestion or indexing.
