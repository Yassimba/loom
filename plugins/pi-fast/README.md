# @yassimba/pi-fast

Use `/fast` in Pi to request priority responses from OpenAI, Codex, and xAI. The plugin sends `service_tier: "priority"` and shows `fast` beside the model name.

Priority may cost more. The provider decides whether to accept the request and how to bill it. The `fast` label means you requested priority; it does not confirm faster service.

## Install and use

```bash
pi install npm:@yassimba/pi-fast
```

Run `/fast` without arguments to turn Fast Mode on or off. To start a session with it on:

```bash
pi --fast
```

Your choice lasts for the session unless you enable `persistState` below.

## Choose which models can use it

By default, Fast Mode can request priority for every model from these Pi providers:

```json
["openai/*", "openai-codex/*", "xai/*"]
```

This includes GPT-6 Astra and future models, so new model names do not need a plugin update. The provider may still reject or ignore priority requests for a particular model or account.

To choose your own models, set `supportedModels` in the settings file. Each entry must be an exact `provider/model` name or `provider/*` for all models from that provider. For example:

```json
{
  "supportedModels": ["openai-codex/*", "openai/gpt-6-astra"]
}
```

Your list replaces the defaults. Use `[]` to allow no models. Patterns such as `openai/gpt-*` are not accepted.

If you switch to a model outside the list, Fast Mode pauses and keeps your on/off choice. It resumes when you select a model the list allows.

## Settings

The plugin reads two files:

- Global settings: `~/.pi/agent/extensions/pi-fast.json`
- Project settings: `.pi/extensions/pi-fast.json`

Project settings override the matching global fields. Omitted fields keep their global value or the default. If neither file exists, the plugin creates the global file with these defaults:

```json
{
  "persistState": false,
  "desiredActive": false,
  "supportedModels": ["openai/*", "openai-codex/*", "xai/*"],
  "footer": {
    "mode": "replace"
  }
}
```

Set `persistState` to `true` to remember your `/fast` choice between sessions. The plugin saves it in `desiredActive`, using the project file if one exists or the global file otherwise. It leaves unreadable or invalid JSON files unchanged when saving a choice.

Run `/reload` after editing settings.

### Where the label appears

Set `footer.mode` to one of these values:

- `replace`: show `fast` beside the model name in the plugin's footer. This is the default.
- `status`: keep Pi's normal footer and add a `fast` status label.
- `off`: hide the label. Fast Mode still requests priority when it is on and the model is allowed.

### Label colors

In `replace` mode, the label uses the theme's thinking-level color, or a dim color when thinking is off. You can set your own colors:

```json
{
  "footer": {
    "darkFastColor": "brand",
    "lightFastColor": "#0066cc",
    "vars": { "brand": "#00ffaa" }
  }
}
```

Use a hex color such as `#00ffaa`, a 256-color index from 0 to 255 (a number or numeric string), a name from `footer.vars`, or `""` for the terminal's default color. Variables can refer to other variables, but must end at a color rather than loop back to themselves.

The plugin uses `lightFastColor` for a theme named `light` (ignoring case), and `darkFastColor` for other themes. Invalid settings produce a warning and leave the global or theme color in use.

## Subagents

When you toggle `/fast`, the plugin sets `PI_FAST_DESIRED` to `1` for on or `0` for off. Pi subagents started afterwards inherit that value. They need this plugin and a model their settings allow. Already-running subagents keep their own choice.

At startup, `--fast` turns Fast Mode on. Otherwise, the plugin uses `PI_FAST_DESIRED` if set, then the saved choice if `persistState` is enabled. With none of these, it starts off.

To check whether a subagent inherited your choice, ask it to print `PI_FAST_DESIRED`.

## Upgrade from pi-openai-fast

The package name, settings filename, and environment variable have changed. The new plugin does not read the old names.

1. Replace the installed package:

   ```bash
   pi remove npm:@yassimba/pi-openai-fast
   pi install npm:@yassimba/pi-fast
   ```

   Add `-l` to both commands for a project-local install. Load only one version: both register `/fast`.

2. Rename `pi-openai-fast.json` to `pi-fast.json` in each global or project settings directory you use. If `pi-fast.json` already exists, merge the settings instead of overwriting it.
3. To use the default providers, remove `supportedModels` from both settings files. Keep it if you want a specific list. Rename an old `active` field to `desiredActive`, and remove old color settings if you want the theme's colors.
4. Replace `PI_OPENAI_FAST_DESIRED` with `PI_FAST_DESIRED` in launch scripts. Restart Pi and its subagents.

If you load the plugin through a local file or symlink, change `plugins/openai-fast/index.ts` to `plugins/pi-fast/index.ts`. For a local package directory, change `plugins/openai-fast` to `plugins/pi-fast`.

## Attribution

Based on [studioarray/pi-openai-fast](https://github.com/studioarray/pi-openai-fast), commit [`e82ed32`](https://github.com/studioarray/pi-openai-fast/commit/e82ed32f1b7c5a946d441d948da33de40da7b04a), under the MIT License. The upstream project was inspired by [pi-better-openai](https://github.com/mattleong/pi-better-openai/).
