// Inline the built bundle into a single self-contained index.html.
// ES modules are blocked over file://, so an un-inlined dist only works when
// served. This makes the report view emailable — and screenshot-able.
import { readFileSync, writeFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const dist = 'dist';
const htmlPath = join(dist, 'index.html');
let html = readFileSync(htmlPath, 'utf8');

const script = /<script type="module"[^>]*src="\.\/(assets\/[^"]+\.js)"[^>]*><\/script>/;
const style = /<link rel="stylesheet"[^>]*href="\.\/(assets\/[^"]+\.css)"[^>]*>/;

const scriptMatch = html.match(script);
const styleMatch = html.match(style);
if (!scriptMatch) throw new Error('no module script found in dist/index.html');

const js = readFileSync(join(dist, scriptMatch[1]), 'utf8');
html = html.replace(script, () => `<script type="module">${js}</script>`);
if (styleMatch) {
  const css = readFileSync(join(dist, styleMatch[1]), 'utf8');
  html = html.replace(style, () => `<style>${css}</style>`);
}
writeFileSync(htmlPath, html);
const bytes = statSync(htmlPath).size;
console.log(`inlined ${htmlPath} (${(bytes / 1024).toFixed(0)} KiB, self-contained)`);
