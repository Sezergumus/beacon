# Beacon app icons

`app-icon.png` is the clean RGBA master. Its rounded corners and outer margin
are transparent. `beacon-icon.jpg` is the original reference and must not be
used directly to regenerate icons: its white background causes edge halos.

Run `npm run icons:generate` from the project root to regenerate the desktop,
Windows Store, iOS and Android icons and sync `public/beacon-icon.png`.
`icon-manifest.json` sets a dark fill for platforms that require a background.
Run `npm run build` to refresh the frontend distribution afterward.

The master was edited using the built-in image generation tool with the
original JPEG as the edit target. Prompt: preserve the cyan glass beacon,
stone base and dark rounded tile; remove the exterior white background,
matte, fringe and shadow; use true transparency and clean antialiased edges;
keep a small transparent safety margin, with no added text or objects.
