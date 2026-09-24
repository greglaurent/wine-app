# barback. identity

- Use the full wordmark for app headers and branded entry points. Its first
  letter is the supplied B symbol with a bottle cutout, followed by `arback.`.
- Use the standalone rounded-square B symbol for favicons and app icons.
- In ordinary prose, contact information, and page titles, write `barback.`
  in lowercase with a normal b. Do not substitute the graphic inside prose.
- `*-light.svg` is charcoal (#221E22) for light surfaces; `*-dark.svg` is
  warm white (#FAF7F2) for dark surfaces. Wordmarks retain the gold (#ECA72C) dot.
- The browser favicon follows the app appearance; the default favicon also
  supports the OS color scheme. The installed app icon uses a purple background.

The PNG sources are cleaned, transparent adaptations of the supplied
`Downloads/barback.png`, prepared with the imagegen skill. They preserve the
intended identity but are generated derivatives, not original vector masters.
The original download is unchanged. Theme variants use the same alpha artwork
so switching appearance does not change the lettering. SVG files embed raster
masks; they are not fully vector outlines.

Regenerate packaged assets with `node web/scripts/brand-assets.mjs`.
Published assets live in `web/public/brand/`.
