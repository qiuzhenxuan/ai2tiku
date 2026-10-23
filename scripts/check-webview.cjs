// Run after pnpm build. Requires Playwright; PLAYWRIGHT_MODULE can select an
// existing installation. Test data replaces Tauri IPC, never the user's database.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const output = path.resolve(process.argv[2] || 'test-results/webview');

async function fixtures() {
  window.testPrefs = {};
  window.testRevision = 1;
  const records = [{ id: 1, title: '下列关于学习方法的说法，正确的是？', answer: 'A', source: 'cache', provider: '本地题库', model: '', promptTokens: 0, completionTokens: 0, totalTokens: 0, durationMs: 12, createdAt: '2026-10-23T08:00:00Z' }];
  const questions = records.map(r => ({ ...r, options: 'A. 主动回顾\nB. 只读不练\nC. 临时突击\nD. 忽略反馈', qtype: 'single', hits: 12, updatedAt: r.createdAt }));
  const status = { running: true, port: 3100, address: 'http://127.0.0.1:3100' };
  records.forEach(record => Object.assign(record, { options: questions[0].options, qtype: 'single', status: 'success', error: '', explanation: '', imageCount: 0 }));
  window.testData = { records, questions, status };
  window.__TAURI_INTERNALS__ = { invoke: async (cmd, args) => {
    if (cmd === 'data_revision') return window.testRevision;
    if (cmd === 'get_ui_pref') return window.testPrefs[args.key] ?? null;
    if (cmd === 'set_ui_pref') { window.testPrefs[args.key] = args.value; return; }
    if (cmd === 'server_status') return status;
    if (cmd === 'get_stats') return { successCount: 123, failureCount: 5, successRate: 123 / 128, questionTotal: 128, cacheHits: 86, aiCalls: 42, hitRate: .672, promptTokens: 18240, completionTokens: 3240, totalTokens: 21480, todayCount: 24, todayTokens: 3120, trend: [{ date: '2026-10-22', count: 12, tokens: 1200 }, { date: '2026-10-23', count: 24, tokens: 3120 }], modelUsage: [{ model: 'glm-4-flash', count: 32, tokens: 15400 }] };
    if (cmd === 'list_records') return { items: records, total: 1, page: 1, pageSize: 15 };
    if (cmd === 'list_questions') return { items: questions, total: 1, page: 1, pageSize: 15 };
    if (['list_providers', 'list_models', 'list_provider_presets'].includes(cmd)) return [];
    if (cmd === 'get_settings') return { port: 3100, matchThreshold: .85, autoCache: true, aiFallback: true, systemPrompt: '', proxyUrl: '' };
    if (cmd === 'default_system_prompt') return '请根据题型和完整选项判断，只返回格式正确的答案。';
    if (cmd === 'browser_info') return { edgeInstalled: true, edgeDefault: false, defaultBrowser: 'Chrome' };
    if (cmd === 'deployment_status') return { server: status, aiModel: null, aiFallback: true, config: JSON.stringify([{ name: 'ai2tiku', url: 'http://127.0.0.1:3100/api/search', method: 'post' }], null, 2) };
    return null;
  } };
}

async function main() {
  await fs.mkdir(output, { recursive: true });
  const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--host', '127.0.0.1', '--port', '4182', '--strictPort'], { cwd: path.resolve(__dirname, '..'), windowsHide: true, stdio: 'pipe' });
  let browser;
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('preview timeout')), 15000);
      server.stdout.on('data', chunk => { if (chunk.toString().includes('Local:')) { clearTimeout(timer); resolve(); } });
      server.on('error', reject);
      server.on('exit', code => { clearTimeout(timer); reject(Error(`preview exited: ${code}`)); });
    });
    browser = await chromium.launch({ headless: true, ...(process.env.EDGE_PATH ? { executablePath: process.env.EDGE_PATH } : {}) });
    const context = await browser.newContext({ viewport: { width: 1160, height: 760 }, reducedMotion: 'no-preference' });
    await context.addInitScript(fixtures);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const base = 'http://127.0.0.1:4182/#/';
    await page.goto(base);
    await page.locator('.page').waitFor();
    const origins = () => page.locator('.rail__item > .rail__glyph').evaluateAll(items => items.map(i => ({ x: i.getBoundingClientRect().x, y: i.getBoundingClientRect().y })));
    const before = await origins();
    assert.equal(await page.getByRole('button', { name: '展开菜单', exact: true }).getAttribute('aria-expanded'), 'false');
    await page.getByRole('button', { name: '展开菜单', exact: true }).click();
    await page.waitForFunction(() => Math.abs(document.querySelector('.rail').getBoundingClientRect().width - 200) < .1);
    assert.deepEqual(await origins(), before);
    await page.getByRole('button', { name: '收起菜单', exact: true }).focus();
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => Math.abs(document.querySelector('.rail').getBoundingClientRect().width - 72) < .1);
    assert.deepEqual(await origins(), before);
    assert.equal(await page.locator('.rail--expanded').count(), 0);

    const toggle = page.locator('.rail__toggle');
    const arrowShift = () => page.locator('.rail__toggle-arrow').evaluate(el => new DOMMatrixReadOnly(getComputedStyle(el).transform).e);
    const toggleBounds = await toggle.boundingBox();
    // End the preceding keyboard interaction before testing pointer-only exit.
    await page.locator('.topbar__title').click();
    await toggle.hover(); await page.waitForTimeout(340);
    assert.ok(await arrowShift() > 2.9, 'Collapsed toggle must guide toward opening');
    assert.deepEqual(await toggle.boundingBox(), toggleBounds, 'Hover must not resize the toggle');
    await toggle.click(); await page.waitForTimeout(340);
    assert.ok(await arrowShift() < -2.9, 'Expanded toggle must guide toward closing');
    await page.locator('.topbar__title').hover(); await page.waitForTimeout(340);
    assert.ok(Math.abs(await arrowShift()) < .1, JSON.stringify(await toggle.evaluate(el => ({ message: 'Pointer exit must restore the toggle', hover: el.matches(':hover'), focus: el.matches(':focus-visible'), arrow: getComputedStyle(el.querySelector('.rail__toggle-arrow')).transform }))));
    await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab');
    assert.equal(await toggle.evaluate(el => el.matches(':focus-visible')), true);
    await page.waitForTimeout(340); assert.ok(await arrowShift() < -2.9, 'Keyboard focus must show the closing cue');
    await toggle.click(); await page.waitForTimeout(340);

    const sidebarMotion = [];
    for (const reverse of [false, false, true]) {
      const timeline = await page.evaluate(reverse => new Promise(resolve => {
        const frames = []; const started = performance.now();
        const rail = document.querySelector('.rail');
        const toggle = document.querySelector('.rail__toggle');
        function sample(time) {
          frames.push({ time: time - started, width: rail.getBoundingClientRect().width,
            iconX: document.querySelector('.rail__item > .rail__glyph').getBoundingClientRect().x });
          if (time - started < 440) requestAnimationFrame(sample); else resolve(frames);
        }
        requestAnimationFrame(sample); toggle.click();
        if (reverse) setTimeout(() => toggle.click(), 80);
      }), reverse);
      const middle = timeline.filter(frame => frame.width > 73 && frame.width < 199);
      assert.ok(middle.length >= 5, 'Sidebar must interpolate, not jump between widths');
      assert.ok(timeline.every(frame => frame.width >= 71.9 && frame.width <= 200.1));
      assert.ok(timeline.every(frame => Math.abs(frame.iconX - before[0].x) < .1), 'Icons must stay aligned during every animation frame');
      const finalWidth = sidebarMotion.length === 0 ? 200 : 72;
      assert.ok(Math.abs(timeline.at(-1).width - finalWidth) < .1);
      sidebarMotion.push({ reverse, frames: timeline.length, intermediateFrames: middle.length, finalWidth });
    }

    const routes = ['', 'deploy', 'ocs', 'providers', 'questions', 'records', 'stats', 'settings'];
    const measurements = [];
    for (const size of [{ width: 1160, height: 760 }, { width: 900, height: 600 }]) {
      await page.setViewportSize(size);
      for (const theme of ['light', 'dark']) for (const expanded of [false, true]) for (const route of routes) {
        await page.goto(base + route);
        await page.locator('.page').waitFor();
        const currentExpanded = await page.locator('.rail__toggle').getAttribute('aria-expanded') === 'true';
        if (currentExpanded !== expanded) await page.locator('.rail__toggle').click();
        await page.waitForFunction(width => Math.abs(document.querySelector('.rail').getBoundingClientRect().width - width) < .1, expanded ? 200 : 72);
        await page.getByRole('button', { name: theme === 'dark' ? '夜间' : '日间', exact: true }).click();
        await page.waitForTimeout(180);
        const dimensions = await page.evaluate(() => ({
          rail: document.querySelector('.rail').getBoundingClientRect().width,
          overflow: document.documentElement.scrollWidth > innerWidth,
          contentOverflow: document.querySelector('.content').scrollWidth > document.querySelector('.content').clientWidth,
          navInside: [...document.querySelectorAll('.rail__item')].every(item => item.getBoundingClientRect().bottom <= innerHeight),
          controls: [...document.querySelectorAll('.btn, .input, .select')].filter(item => item.getClientRects().length).map(item => ({ tag: item.className, height: item.getBoundingClientRect().height })),
        }));
        assert.equal(dimensions.rail, expanded ? 200 : 72);
        assert.equal(dimensions.overflow, false, `${route}: document overflow`);
        assert.equal(dimensions.contentOverflow, false, `${route}: content overflow`);
        assert.equal(dimensions.navInside, true, 'all eight navigation items must be reachable');
        for (const control of dimensions.controls) assert.equal(control.height, 34, JSON.stringify({ route, ...control }));
        const name = `${route || 'dashboard'}-${theme}-${expanded ? 'expanded' : 'collapsed'}-${size.width}`;
        await page.screenshot({ path: path.join(output, name + '.png') });
        measurements.push({ name, ...dimensions });
      }
    }
    await page.setViewportSize({ width: 1160, height: 760 });
    await page.goto(base + 'questions');
    await page.locator('.page').waitFor();
    const primary = page.locator('.content .btn--primary').first();
    const idle = await primary.evaluate(el => ({ background: getComputedStyle(el).backgroundColor, rect: el.getBoundingClientRect().toJSON() }));
    await primary.hover(); await page.waitForTimeout(180);
    const hover = await primary.evaluate(el => ({ background: getComputedStyle(el).backgroundColor, rect: el.getBoundingClientRect().toJSON(), cursor: getComputedStyle(el).cursor }));
    assert.notEqual(hover.background, idle.background); assert.deepEqual(hover.rect, idle.rect); assert.equal(hover.cursor, 'pointer');
    await page.mouse.down(); await page.waitForTimeout(180);
    assert.notEqual(await primary.evaluate(el => getComputedStyle(el).backgroundColor), hover.background);
    await page.mouse.up();
    await page.locator('.modal').waitFor();
    for (const selector of ['body', '.modal', '.modal .textarea']) {
      assert.equal(await page.locator(selector).first().evaluate(el => !el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 2 }))), true, selector);
    }
    const input = page.locator('.modal .textarea').first();
    await input.fill('编辑快捷键可用'); await input.press('ControlOrMeta+A'); await input.press('Backspace');
    assert.equal(await input.inputValue(), '');
    await page.getByRole('button', { name: '取消', exact: true }).click();
    const first = page.locator('.rail__item').first();
    await first.focus(); await page.keyboard.press('End');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('href')), '#/settings');
    await page.keyboard.press('ArrowDown'); assert.equal(await first.evaluate(el => document.activeElement === el), true);
    await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter');
    await page.waitForURL('**/#/deploy');
    await page.keyboard.press('Escape');

    const rocket = page.locator('.rail__item--rocket');
    await rocket.hover(); await page.waitForTimeout(1500);
    assert.equal(await page.locator('.rocket-mark__exhaust i').count(), 72);
    const jet = await page.locator('.rocket-mark__exhaust').evaluate(el => ({ opacity: getComputedStyle(el).opacity, particle: getComputedStyle(el.querySelector('i')).animationPlayState, lift: getComputedStyle(el.parentElement).transform }));
    assert.equal(jet.opacity, '1'); assert.equal(jet.particle, 'running'); assert.match(jet.lift, /-4\)/);
    const frames = await page.evaluate(() => new Promise(resolve => {
      const times = []; const start = performance.now();
      function tick(time) { times.push(time); if (time - start < 2000) requestAnimationFrame(tick); else resolve(times); }
      requestAnimationFrame(tick);
    }));
    const intervals = frames.slice(1).map((time, i) => time - frames[i]).sort((a, b) => a - b);
    const cadence = { frames: frames.length, medianMs: intervals[Math.floor(intervals.length / 2)], p95Ms: intervals[Math.floor(intervals.length * .95)] };
    assert.ok(cadence.medianMs < 25 && cadence.p95Ms < 50, JSON.stringify(cadence));
    await page.screenshot({ path: path.join(output, 'rocket-hover.png') });
    await page.mouse.move(700, 100); await page.locator('.topbar__title').click(); await page.waitForTimeout(1500);
    assert.equal(await page.locator('.rocket-mark__exhaust').evaluate(el => getComputedStyle(el).opacity), '0');
    assert.equal(await page.locator('.rocket-mark__exhaust i').first().evaluate(el => getComputedStyle(el).animationPlayState), 'paused');
    await page.emulateMedia({ reducedMotion: 'reduce' }); await rocket.hover();
    assert.equal(await page.locator('.rocket-mark__exhaust').evaluate(el => getComputedStyle(el).display), 'none');
    assert.equal(await page.locator('.rocket-mark').evaluate(el => getComputedStyle(el).transform), 'none');
    assert.equal(await page.locator('.app-shell').evaluate(el => getComputedStyle(el).transitionDuration), '0s');
    await toggle.hover();
    assert.ok(Math.abs(await arrowShift()) < .1, 'Reduced motion must keep the toggle icon still');
    assert.deepEqual(errors, []);
    const result = { passed: true, screenshots: 65, pages: 8, themes: 2, sidebarStates: 2, sidebarMotion, toggleDirectionCues: true, toggleFocusAndReset: true, sizes: [1160, 900], contextMenuPrevented: true, editingShortcuts: true, controlsHeight: 34, particleCount: 72, reducedMotion: true, cadence, errors, scope: 'Production WebView frontend in Edge; isolated mocked IPC; no paid model calls', measurements };
    await fs.writeFile(path.join(output, 'result.json'), JSON.stringify(result, null, 2));
    console.log(JSON.stringify({ ...result, measurements: undefined }));
  } finally { if (browser) await browser.close(); server.kill(); }
}
if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1; });
module.exports = { fixtures };
