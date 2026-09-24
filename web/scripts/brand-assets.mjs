// Package the cleaned artwork as theme-colored SVG assets. The embedded PNGs
// supply alpha masks; CSS-independent colors also work in downloaded SVG files.
import { readFile, writeFile } from 'node:fs/promises';

const root = new URL('../../', import.meta.url);
const wordmark = (await readFile(new URL('docs/brand/wordmark-source.png', root))).toString('base64');
const symbol = (await readFile(new URL('docs/brand/symbol-source.png', root))).toString('base64');
function svg(kind, color, adaptive = false) {
  const word = kind === 'wordmark';
  const box = word ? '50 145 2080 415' : '0 0 1254 1254';
  const size = word ? 'width="2172" height="724"' : 'width="1254" height="1254"';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${box}" role="img" aria-label="barback.">
  ${adaptive ? '<style>.ink{fill:#221e22}@media(prefers-color-scheme:dark){.ink{fill:#faf7f2}}</style>' : ''}
  <defs><mask id="art" maskUnits="userSpaceOnUse" x="0" y="0" ${size} style="mask-type:alpha"><image ${size} href="data:image/png;base64,${word ? wordmark : symbol}"/></mask></defs>
  <rect class="ink" width="${word ? 1995 : 1254}" height="1254" fill="${color}" mask="url(#art)"/>
  ${word ? '<circle cx="2055" cy="490" r="56" fill="#eca72c"/>' : ''}
</svg>\n`;
}
for (const kind of ['wordmark', 'symbol']) {
  for (const [theme, color] of [['light', '#221e22'], ['dark', '#faf7f2']]) {
    await writeFile(new URL(`web/public/brand/${kind}-${theme}.svg`, root), svg(kind, color));
  }
}
await writeFile(new URL('web/public/icon.svg', root), svg('symbol', '#221e22', true));
await writeFile(new URL('web/public/brand/app-icon.svg', root), svg('symbol', '#faf7f2').replace('<defs>', '<rect width="1254" height="1254" rx="210" fill="#44355b"/><defs>'));
