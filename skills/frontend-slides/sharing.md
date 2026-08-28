# Share & Export

Read this when the user accepts the offer to share a finished deck — a live URL, a PDF, or both.

## Deploy to a live URL (Vercel)

A free host; the link works on any device and stays live until the user deletes it.

1. `npx vercel --version` — if the command is missing, install Node.js first (`brew install node`, or nodejs.org).
2. `npx vercel whoami` — if it reports no user, walk them through signing up: send them to https://vercel.com/signup (GitHub, Google, or email — whichever is fastest), then `vercel login`, which opens a browser to authorise, then `vercel whoami` to confirm. Wait for them to confirm before deploying.
3. `bash scripts/deploy.sh <path>` — the path is either a folder containing `index.html` or a single HTML file.
4. Give the user the URL from the script output, and tell them the free tier costs nothing and that https://vercel.com/dashboard is where they delete the project later.

**Assets travel with the HTML.** The script bundles files referenced by `src="..."`. A file referenced only from CSS `background-image`, or by an unusual path, can be missed — so open the deployed URL and confirm every image loads. When a deck has more than a couple of assets, put the HTML and its assets in one folder and deploy the folder: `bash scripts/deploy.sh ./my-deck/`. The whole folder uploads as-is, which is the reliable path.

**Filenames.** Spaces work — Vercel encodes them as `%20` — but hyphens avoid the whole question. If an image breaks and the name has spaces, renaming is the fix.

**Redeploying** the same presentation overwrites the previous one at the same URL, so a link already shared keeps working.

## Export to PDF

Each slide is screenshotted and the shots are combined into one PDF — email, Slack, Notion, print. Animations resolve to their final visual state; say so to the user, so a static file is expected rather than a surprise.

```bash
bash scripts/export-pdf.sh <path-to-html> [output.pdf]
```

The PDF lands next to the HTML when no output path is given, and the script opens it. Tell the user its location and size.

**First run takes 30–60 seconds.** The script installs Playwright and downloads Chromium (~150MB) into a temp directory. Later exports in the same session are fast. If Chromium fails to download, run `npx playwright install chromium`; if that fails too, it is usually a network or firewall block, so suggest another network.

**Slides are found by `.slide`.** Decks from this skill always use that class. An externally authored deck that names slides something else makes the script report "0 slides found".

**Images need relative paths.** The script serves the HTML's parent directory over HTTP, so `src="photo.png"` resolves — spaces in the name included. An absolute filesystem path (`src="/Users/name/photo.png"`) does not load. Generated decks are always relative; a converted or user-supplied deck may need fixing first.

**Large decks make large PDFs** — each slide is a full 1920×1080 PNG, so 18 slides can reach ~20MB. Past 10MB, ask: *"The PDF is [size]. Would you like me to compress it? It'll look slightly less sharp but the file will be much smaller."* On yes, re-run with `--compact`, which renders at 1280×720 and typically cuts 50–70% for little visible difference.
