# Teams slots (personal prototype)

**The LLM does not write a script.** This folder *is* the script. An agent parses “find a spot for me and Winand next week” and runs:

```bash
node slots.mjs find Winand --when next-week --duration 30 --top 3
```

Fully automatable after one setup. The only fuzzy bit is “generic block vs real meeting”; that is a keyword list today, not an LLM call.

## Ranking

Worst person wins. Real meetings and OOF are never overbooked. Then:

1. Earliest **all free** inside the hours (default 09:00–17:00)
2. Same window, over a **tentative**
3. Same window, over a **generic** block (Focus time, Lunch, Busy, Hold, …)
4. Earliest **outside** those hours (07:00–20:00 weekdays) with the same free → tentative → generic ladder

Works for 2 or 6 people: `getSchedule` takes up to 20 addresses.

## One-time setup

`op` is installed. The service account token is not in this shell yet.

```bash
cd prototypes/teams-slots
cp .env.example .env
# fill OP_SERVICE_ACCOUNT_TOKEN, OP_TEAMS_VAULT, OP_TEAMS_ITEM
npm install && npx playwright install chromium
npm run setup
```

`--setup` opens Chrome, fills Microsoft email/password from 1Password, then the TOTP at the moment it is needed. The Graph bearer is cached in `.cache/graph.json` (mode 0600). Later `find` calls skip the browser until the token expires.

Number-matching Authenticator prompts cannot be filled. The 1Password item must be the TOTP method (or Microsoft must offer “use a verification code”).

## Find

```bash
npm start -- find Winand --when next-week
npm start -- find Winand Alex --duration 60 --hours 9-17 --json
npm start -- find  winand@contoso.com --when 5
```

Names go through Graph `/me/people`. You are always included.

## If it fails

- `op` errors: token/vault/item, or the service account cannot read that vault.
- Login hangs: `npm run setup -- --headed` and watch; Conditional Access may want a real device.
- `Could not resolve "Winand"`: pass the email.
- `getSchedule` 403: tenant hides free/busy.

The durable replacement is still an Entra app + MSAL. This path exists so an agent can book time without that.
