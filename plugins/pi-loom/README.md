# pi-loom

Loom's woven logo in Pi's header, with a startup notice when a newer Loom CLI is ready to install.

## Try it

From a checkout of this repository, with Pi 0.85.1 or newer:

```sh
pi -e ./plugins/pi-loom/index.ts
```

To keep it installed:

```sh
pi install ./plugins/pi-loom
```

Restart Pi after installing to run the startup check. The package is also registered in Loom's setup catalog for publication; it is not added to existing installations automatically.

## Behavior

- Replaces Pi's header with a purple, 24-column Unicode braille rendering of the supplied Loom logo, with the name aligned at the top. Narrow terminals show just **Loom**. Pi's other UI and native update notices remain unchanged.
- At interactive process startup, runs `loom --version` and reads the Loom executable pin from the [published manifest](https://github.com/Yassimba/loom/blob/main/manifest/loom.toml). It does not use GitHub's latest release, which could belong to a different component.
- If that stable version is newer, shows: `Loom update available: VERSION. Run loom update or use the /loom skill to update.` The skill must be installed separately; `loom update` always remains the direct command.
- Never installs anything or changes Pi settings. Checks run in the background, with a 1.5-second command timeout and a four-second overall deadline. Missing Loom, unrecognized/development versions, and network failures stay quiet.
- Skips checks in headless modes and with `PI_OFFLINE=1`, `true`, or `yes`. Switching, forking, or reloading sessions rebrands the header without repeating the check. Shutdown cancels pending work and suppresses late notices.

Only one extension can own Pi's header; another header extension may replace this one. Remove pi-loom with `pi remove ./plugins/pi-loom` (from the same checkout) and reload to return to Pi's normal header.

## Development

From the repository root:

```sh
node --experimental-strip-types --test test/pi-loom.test.ts
npm run check
npm run audit
```
