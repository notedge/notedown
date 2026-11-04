// Inspect a rendered chart page inside the Chromium that @playwright/test ships.
// No `channel` and no `executablePath`: the system browser is never touched.
import { chromium } from '@playwright/test';

const at = process.argv.indexOf('--html');
if (at === -1 || at + 1 >= process.argv.length) {
  throw new Error('missing --html <path>');
}
const html = process.argv[at + 1];

const browser = await chromium.launch();
const modes = {};
const ids = [];

for (const colorScheme of ['light', 'dark']) {
  const context = await browser.newContext({
    viewport: { width: 960, height: 320 },
    deviceScaleFactor: 1,
    colorScheme,
    locale: 'en-US',
    timezoneId: 'UTC',
    reducedMotion: 'reduce',
  });
  const page = await context.newPage();
  await page.goto(`file://${html.replaceAll('\\', '/')}`, { waitUntil: 'load' });
  const probe = await page.evaluate(() => {
    const read = (selector, property) => {
      const element = document.querySelector(selector);
      return element ? getComputedStyle(element)[property] : null;
    };
    return {
      ids: Array.from(document.querySelectorAll('[id]')).map((element) => element.id),
      tickFill: read('.chart text.chart-tick', 'fill'),
      axisStroke: read('.chart line.chart-axis', 'stroke'),
    };
  });
  if (colorScheme === 'light') ids.push(...probe.ids);
  modes[colorScheme] = { tickFill: probe.tickFill, axisStroke: probe.axisStroke };
  await context.close();
}

await browser.close();
process.stdout.write(`META ${JSON.stringify({ ids, modes })}\n`);
