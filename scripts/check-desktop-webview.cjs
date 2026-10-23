// Connect only to a separately launched test EXE with --data-dir and a local
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9227.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');

(async () => {
  const output = path.resolve(process.argv[3] || 'test-results/desktop-webview');
  await fs.mkdir(output, { recursive: true });
  const browser = await chromium.connectOverCDP(process.argv[2] || 'http://127.0.0.1:9227');
  try {
    const page = browser.contexts().flatMap(context => context.pages()).find(page => /tauri\.localhost|tauri:\/\//.test(page.url()));
    assert.ok(page, 'Expected an isolated Tauri test window');
    const root = page.url().split('#')[0];
    await page.locator('.page').waitFor();
    await page.reload();
    await page.getByRole('button', { name: '展开菜单', exact: true }).click();
    assert.equal(await page.locator('.rail').evaluate(el => el.getBoundingClientRect().width), 200);
    await page.screenshot({ path: path.join(output, 'desktop-expanded.png') });
    await page.getByRole('button', { name: '收起菜单', exact: true }).click();
    await page.goto(root + '#/questions');
    await page.getByRole('button', { name: '新增题目', exact: false }).click();
    await page.locator('.modal').waitFor();
    for (const selector of ['body', '.modal', '#q-title']) {
      assert.equal(await page.locator(selector).evaluate(el => !el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 2 }))), true);
    }
    await page.locator('#q-title').fill('仅在独立测试窗口编辑，不保存');
    await page.locator('#q-title').press('ControlOrMeta+A');
    await page.locator('#q-title').press('Backspace');
    assert.equal(await page.locator('#q-title').inputValue(), '');
    await page.getByRole('button', { name: '取消', exact: true }).click();
    await page.locator('.rail__item--rocket').hover();
    await page.waitForTimeout(1500);
    assert.equal(await page.locator('.rocket-mark__exhaust').evaluate(el => getComputedStyle(el).opacity), '1');
    assert.equal(await page.locator('.rocket-mark__exhaust i').count(), 72);
    await page.screenshot({ path: path.join(output, 'desktop-rocket.png') });
    const samples = await page.evaluate(() => new Promise(resolve => {
      const frames = []; const start = performance.now();
      const tick = time => { frames.push(time); if (time - start < 2000) requestAnimationFrame(tick); else resolve(frames); };
      requestAnimationFrame(tick);
    }));
    const intervals = samples.slice(1).map((time, index) => time - samples[index]).sort((a, b) => a - b);
    const result = { passed: true, actualTauriRelease: true, contextMenuPrevented: true, editingShortcuts: true, sidebarToggle: true, particles: 72, frameSamples: samples.length, medianFrameMs: intervals[Math.floor(intervals.length / 2)], p95FrameMs: intervals[Math.floor(intervals.length * .95)], scope: 'Actual Windows Tauri/WebView2 Release, isolated --data-dir, no paid calls and no test IPC mocks' };
    assert.ok(result.medianFrameMs < 25 && result.p95FrameMs < 50, JSON.stringify(result));
    await fs.writeFile(path.join(output, 'desktop-result.json'), JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result));
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
