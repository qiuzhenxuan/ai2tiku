// Headless regression checks. No screenshots, desktop windows or real IPC.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { fixtures } = require('./check-webview.cjs');
const { spawn } = require('node:child_process');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');

async function main() {
  const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--host', '127.0.0.1', '--port', '4184', '--strictPort'], { cwd: path.resolve(__dirname, '..'), windowsHide: true, stdio: 'pipe' });
  let browser;
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('preview timeout')), 15000);
      server.stdout.on('data', chunk => { if (String(chunk).includes('Local:')) { clearTimeout(timer); resolve(); } });
      server.on('error', reject);
    });
    browser = await chromium.launch({ headless: true, ...(process.env.EDGE_PATH ? { executablePath: process.env.EDGE_PATH } : {}) });
    const context = await browser.newContext({ viewport: { width: 1160, height: 760 }, reducedMotion: 'no-preference' });
    await context.addInitScript(fixtures);
    await context.addInitScript(() => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
        if (cmd === 'list_providers') return [{ id: 1, name: '测试供应商', baseUrl: 'http://example.invalid', apiKey: '', enabled: true }];
        if (cmd === 'list_models') return [{ id: 1, providerId: 1, model: 'test', displayName: '测试模型', enabled: true, isDefault: true, temperature: .3, maxTokens: 100 }];
        if (cmd === 'test_search') {
          await new Promise(resolve => setTimeout(resolve, 400));
          return { code: 1, answer: 'B', source: 'cache', durationMs: 10, totalTokens: 0 };
        }
        return invoke(cmd, args);
      };
    });
    const page = await context.newPage(), errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const base = 'http://127.0.0.1:4184/#/';
    await page.goto(base); await page.locator('.page').waitFor();
    const dashboard = page.locator('.rail__item--dashboard'), rocket = page.locator('.rail__item--rocket');
    await dashboard.hover(); await dashboard.click(); await page.mouse.move(700, 100);
    assert.equal(await page.locator('.rail__wrap.is-open').count(), 0, 'Pointer click must dismiss tooltip');
    await dashboard.hover(); await dashboard.focus(); await page.keyboard.press('ArrowDown');
    assert.equal(await rocket.evaluate(el => el === document.activeElement), true);
    assert.equal(await rocket.getAttribute('aria-describedby'), 'nav-tip-deploy', 'Keyboard must supersede parked pointer');
    await page.keyboard.press('Escape'); assert.equal(await page.locator('.rail__wrap.is-open').count(), 0);
    await dashboard.hover(); assert.equal(await dashboard.getAttribute('aria-describedby'), 'nav-tip-dashboard');

    const nav = ['dashboard', 'rocket', 'plug', 'sparkles', 'database', 'list', 'chart', 'settings'];
    const state = selector => page.locator(selector).evaluate(el => ({ rect: el.getBoundingClientRect().toJSON(), svg: getComputedStyle(el.querySelector('svg')).transform,
      parts: [...el.querySelectorAll('svg > *')].map(part => ({ transform: getComputedStyle(part).transform, origin: getComputedStyle(part).transformOrigin })),
      lift: el.querySelector('.rocket-mark') ? getComputedStyle(el.querySelector('.rocket-mark')).transform : null }));
    for (const name of nav) {
      const selector = '.rail__item--' + name, duration = name === 'rocket' ? 1450 : 750;
      await page.locator('.topbar__title').click(); await page.mouse.move(700, 100); await page.waitForTimeout(duration);
      const idle = await state(selector);
      await page.locator(selector).hover(); await page.waitForTimeout(duration); const hover = await state(selector);
      assert.deepEqual(hover.rect, idle.rect, name + ': stable button bounds');
      assert.deepEqual(hover.parts.map(p => p.origin), idle.parts.map(p => p.origin), name + ': fixed transform origins');
      await page.mouse.move(700, 100); await page.waitForTimeout(duration); assert.deepEqual(await state(selector), idle, name + ': restores');
      await page.keyboard.press('Tab'); await page.locator(selector).focus(); await page.waitForTimeout(duration); assert.deepEqual(await state(selector), hover, name + ': keyboard matches hover');
    }
    for (const theme of ['light', 'dark']) {
      await page.getByRole('button', { name: theme === 'light' ? '日间' : '夜间', exact: true }).click();
      await page.waitForTimeout(200);
      const color = () => dashboard.evaluate(el => getComputedStyle(el).backgroundColor);
      const idle = await color(); await dashboard.hover(); await page.waitForTimeout(200);
      const hover = await color(); assert.notEqual(hover, idle);
      await page.mouse.down(); await page.waitForTimeout(200); assert.notEqual(await color(), hover); await page.mouse.up();
    }
    await page.locator('.topbar__title').click(); await rocket.hover(); await page.waitForTimeout(650);
    const runningCount = () => page.locator('.rocket-mark__exhaust i').evaluateAll(es => es.filter(e => getComputedStyle(e).animationPlayState === 'running').length);
    assert.equal(await runningCount(), 72);
    await page.mouse.move(700, 100); await page.waitForTimeout(60);
    assert.ok(await runningCount() > 0, 'Particle flight must continue during shutdown');
    await rocket.hover(); await page.waitForTimeout(100); assert.equal(await runningCount(), 72, 'Rapid re-entry restores all lanes');
    await page.mouse.move(700, 100); await page.waitForTimeout(650); assert.equal(await runningCount(), 0);
    await rocket.hover(); await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, get: () => true }); document.dispatchEvent(new Event('visibilitychange')); });
    await page.waitForTimeout(30); assert.equal(await runningCount(), 0, 'Hidden page pauses particles');
    await page.evaluate(() => { delete document.hidden; document.dispatchEvent(new Event('visibilitychange')); });
    await page.waitForTimeout(30); assert.equal(await runningCount(), 72);

    await page.goto(base + 'providers'); await page.getByRole('button', { name: '编辑供应商：测试供应商', exact: true }).waitFor();
    const controls = await page.locator('.table .btn--icon').evaluateAll(es => es.map(e => ({ width: e.getBoundingClientRect().width, height: e.getBoundingClientRect().height, label: e.getAttribute('aria-label'), title: e.title })));
    assert.equal(controls.length, 4); controls.forEach(c => { assert.equal(c.width, 34); assert.equal(c.height, 34); assert.ok(c.label && c.title); });
    await page.goto(base + 'questions'); await page.locator('.cell-link').first().waitFor();
    const add = page.getByRole('button', { name: '新增题目', exact: true });
    await add.click(); await page.getByRole('dialog').waitFor(); await page.waitForTimeout(250);
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('#app').evaluate(el => el.inert), false, 'Closing releases background immediately');
    assert.equal(await page.locator('.dialog-leave-active').evaluate(el => el.inert && getComputedStyle(el).pointerEvents === 'none'), true);
    assert.equal(await add.evaluate(el => document.activeElement === el), true);
    await add.click(); await page.getByRole('dialog').waitFor(); await page.waitForTimeout(250);
    assert.equal(await page.getByRole('dialog').count(), 1, 'Quick reopen leaves only one interactive dialog');
    await page.keyboard.press('Escape'); await page.locator('.modal-mask').waitFor({ state: 'detached' });
    await page.locator('.cell-link').first().click(); await page.getByRole('dialog').waitFor();
    await page.keyboard.press('Escape'); await page.locator('.modal-mask').waitFor({ state: 'detached' });
    assert.equal(await page.locator('.cell-link').first().evaluate(el => document.activeElement === el), true);

    await page.goto(base + 'ocs'); await page.getByRole('button', { name: '开始测试', exact: true }).click();
    await page.locator('.result').waitFor();
    assert.ok(await page.locator('.result').evaluate(el => getComputedStyle(el).animationName.startsWith('result-rise')));
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.getByRole('button', { name: '开始测试', exact: true }).click();
    assert.equal(await page.locator('.spin').evaluate(el => getComputedStyle(el).animationName), 'none');
    await page.locator('.result').waitFor(); assert.equal(await page.locator('.result').evaluate(el => getComputedStyle(el).animationName), 'none');
    assert.equal(await page.locator('.dot--pulse').first().evaluate(el => getComputedStyle(el).animationName), 'none');
    await page.getByRole('button', { name: '复制地址', exact: false }).click();
    await page.locator('.toast').first().waitFor(); assert.equal(await page.locator('.toast').first().evaluate(el => getComputedStyle(el).animationName), 'none');
    await page.goto(base + 'questions'); await page.getByRole('button', { name: '新增题目', exact: true }).click();
    const reduced = await page.locator('.modal-mask, .modal').evaluateAll(es => es.map(e => ({ animation: getComputedStyle(e).animationName, transition: getComputedStyle(e).transitionDuration })));
    reduced.forEach(e => { assert.equal(e.animation, 'none'); assert.equal(e.transition, '0s'); });
    await page.keyboard.press('Escape'); await page.locator('.modal-mask').waitFor({ state: 'detached' });
    await rocket.hover(); assert.equal(await page.locator('.rocket-mark__exhaust').evaluate(el => getComputedStyle(el).display), 'none');
    assert.deepEqual(errors, []);
    const result = { passed: true, navIcons: nav.length, tooltipInputPriority: true, pointerClickDismissal: true, fixedTransformOrigins: true, activeFeedbackBothThemes: true, rocketDrainAndReentry: true, hiddenParticlesPaused: true, named34pxIconButtons: controls.length, dialogExitAndQuickReopen: true, focusRestore: true, ocsResultMotion: true, globalReducedMotion: true, screenshots: 0, errors };
    await fs.mkdir('test-results/interactions', { recursive: true }); await fs.writeFile('test-results/interactions/result.json', JSON.stringify(result, null, 2)); console.log(JSON.stringify(result));
  } finally { try { if (browser) await browser.close(); } finally { server.kill(); } }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
