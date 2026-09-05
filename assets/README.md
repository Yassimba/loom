# Loom branding

- `loom-logo.svg`: unchanged copy of the supplied `Loom Logo (1).svg`, used in the repository README.
- `loom-icon.png`: transparent 512 × 512 PNG export of the same logo, for icon use.

Regenerate the icon with librsvg:

```sh
rsvg-convert -w 512 -h 512 assets/loom-logo.svg -o assets/loom-icon.png
```

GitHub repositories do not have a separate avatar. These files do not change an account or organization avatar. For a repository link-preview image, place the logo on a 1280 × 640 canvas and upload it manually under **Settings → General → Social preview**. No remote settings are changed by this repository.
