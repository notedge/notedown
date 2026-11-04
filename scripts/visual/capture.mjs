#!/usr/bin/env node
// Rasterize one static HTML page with the bundled Playwright Chromium.
//
// The gate never launches a system browser: `chromium.launch()` resolves the
// browser that `npx playwright install chromium` put next to this workspace,
// and the identity line below is the only thing the Rust gates pin.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const workspace = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const playwrightPackage = JSON.parse(fs.readFileSync(path.join(workspace, 'node_modules', '@playwright', 'test', 'package.json'), 'utf8'));
const registry = JSON.parse(fs.readFileSync(path.join(workspace, 'node_modules', 'playwright-core', 'browsers.json'), 'utf8'));
const chromiumEntry = registry.browsers.find((entry) => entry.name === 'chromium');
const shellEntry = registry.browsers.find((entry) => entry.name === 'chromium-headless-shell');

function argument(name) {
  const at = process.argv.indexOf(name);
  if (at === -1 || at + 1 >= process.argv.length) {
    throw new Error(`missing ${name}`);
  }
  return process.argv[at + 1];
}

const html = path.resolve(argument('--html'));
const png = path.resolve(argument('--png'));
const width = Number(argument('--width'));
const height = Number(argument('--height'));
const deviceScaleFactor = Number(argument('--dpr') || '1');
const colorScheme = argument('--color-scheme') || 'light';

if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0) {
  throw new Error('--width and --height must be positive integers');
}
if (!html.endsWith('.html')) {
  throw new Error('--html must point at an .html file');
}

const browser = await chromium.launch();
const build = browser.version();
const context = await browser.newContext({
  viewport: { width, height },
  deviceScaleFactor,
  colorScheme,
  locale: 'en-US',
  timezoneId: 'UTC',
  reducedMotion: 'reduce',
});
const page = await context.newPage();
await page.goto(`file://${html.replaceAll('\\', '/')}`, { waitUntil: 'load' });
await page.screenshot({ path: png });
await context.close();
await browser.close();

if (!fs.existsSync(png) || fs.statSync(png).size === 0) {
  throw new Error(`no screenshot at ${png}`);
}

process.stdout.write(
  `META playwright=${playwrightPackage.version} chromium=${build} revision=${chromiumEntry.revision} headless-shell=${shellEntry.revision} dpr=${deviceScaleFactor} color-scheme=${colorScheme}\n`,
);
