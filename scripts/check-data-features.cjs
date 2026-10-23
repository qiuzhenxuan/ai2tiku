// Production UI integration checks with isolated IPC fixtures. Backend validation
// and transactions are tested separately by cargo test; no real model calls.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { fixtures } = require('./check-webview.cjs');
const { spawn } = require('node:child_process');
const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { screenshotArtifacts } = require('./screenshot-artifacts.cjs');
const output = path.resolve(process.argv[2] || 'test-results/data-features');
const artifacts = screenshotArtifacts(output);

async function featureFixtures() {
  const original = window.__TAURI_INTERNALS__.invoke;
  window.testCalls = [];
  window.testFile = ['demo.json', JSON.stringify([{ title: '导入的新题', qtype: 'single', options: 'A. 甲\nB. 乙', answer: 'A' }])];
  window.testPreview = { total: 1, valid: 1, invalid: 0, newCount: 1, duplicates: 0, conflicts: 0, fileDuplicates: 0, fileConflicts: 0, existing: 1, errors: [], snapshot: 'test' };
  window.testFailRecords = location.hash.includes('initial-failure');
  const long = '这是一道很长的测试题，请完整阅读题干后结合全部选项判断。'.repeat(40);
  window.testData.records[0] = { ...window.testData.records[0], title: long, answer: '', source: 'ai', status: 'failure', error: '模型请求超时：未得到有效答案，请检查网络。', explanation: '', options: 'A. ' + '选项一的完整内容。'.repeat(30) + '\nB. 第二个选项\nC. 第三个选项\nD. 第四个选项', qtype: 'single', imageCount: 0 };
  window.testData.questions[0].title = long;
  window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    window.testCalls.push({ cmd, args });
    if (cmd === 'list_records') {
      if (window.testFailRecords) throw Error('测试断连');
      return { items: window.testData.records.slice(0, args.pageSize), total: window.testData.records.length, page: args.page, pageSize: args.pageSize };
    }
    if (cmd === 'list_questions') return { items: window.testData.questions, total: window.testData.questions.length, page: args.page, pageSize: args.pageSize };
    if (cmd === 'get_stats') return { ...await original(cmd, args), successCount: 4, failureCount: window.testData.records.length, successRate: 4 / (4 + window.testData.records.length) };
    if (cmd === 'export_question_file') return 'C:\\test\\question-backup.json';
    if (cmd === 'pick_question_file') return window.testFile;
    if (cmd === 'preview_question_import') return window.testPreview;
    if (cmd === 'apply_question_import') {
      window.testData.questions.push({ ...window.testData.questions[0], id: 2, title: '导入的新题' }); window.testRevision++;
      return { imported: 1, skipped: 0, total: 2, backupPath: args.mode === 'replace' ? 'C:\\test\\backups\\before-restore.json' : '' };
    }
    if (cmd === 'delete_question') { window.testData.questions = window.testData.questions.filter(q => q.id !== args.id); window.testRevision++; return; }
    if (cmd === 'clear_records') { window.testData.records = []; window.testRevision++; return; }
    if (cmd === 'run_diagnostics') return { checkedAt: '2026-10-23 12:00:00', modelCalled: args.includeModel, tokens: args.includeModel ? 5 : 0, items: [
      { name: '本地接口', status: 'pass', detail: '127.0.0.1 本地服务响应正常', durationMs: 2 },
      { name: '默认模型', status: 'warn', detail: '测试配置，尚未验证调用权限', durationMs: 0 },
      { name: '模型实际调用', status: args.includeModel ? 'pass' : 'skip', detail: args.includeModel ? '调用正常，本次 5 Token' : '未调用模型，不产生模型 Token', durationMs: args.includeModel ? 50 : 0 },
    ] };
    return original(cmd, args);
  };
}

(async () => {
  await fs.mkdir(output, { recursive: true });
  const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--host', '127.0.0.1', '--port', '4183', '--strictPort'], { cwd: path.resolve(__dirname, '..'), windowsHide: true, stdio: 'pipe' });
  let browser;
  try {
    await new Promise((resolve, reject) => { const timer = setTimeout(() => reject(Error('preview timeout')), 15000); server.stdout.on('data', data => { if (data.toString().includes('Local:')) { clearTimeout(timer); resolve(); } }); server.on('error', reject); server.on('exit', code => { clearTimeout(timer); reject(Error('preview exit ' + code)); }); });
    browser = await chromium.launch({ headless: true, ...(process.env.EDGE_PATH ? { executablePath: process.env.EDGE_PATH } : {}) });
    const context = await browser.newContext({ viewport: { width: 1160, height: 760 } });
    await context.addInitScript(fixtures); await context.addInitScript(featureFixtures);
    const page = await context.newPage(); const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.on('dialog', dialog => { errors.push('Unexpected browser dialog: ' + dialog.type()); void dialog.dismiss(); });
    const base = 'http://127.0.0.1:4183/#/';
    const calls = cmd => page.evaluate(cmd => window.testCalls.filter(call => call.cmd === cmd).length, cmd);
    await page.goto(base + 'records'); await page.locator('.cell-link').first().waitFor();
    const count = await calls('list_records'); await page.waitForTimeout(1300); assert.equal(await calls('list_records'), count, 'Unchanged revision must not reload records');
    await page.evaluate(() => { window.testData.records.push({ ...window.testData.records[0], id: 2, title: '新到达的失败请求' }); window.testRevision++; });
    await page.waitForFunction(() => document.querySelectorAll('.table tbody tr').length === 2);
    await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, get: () => true }); document.dispatchEvent(new Event('visibilitychange')); });
    const hiddenCalls = await calls('data_revision'); await page.waitForTimeout(1300); assert.equal(await calls('data_revision'), hiddenCalls, 'Hidden page must suspend polling');
    await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, get: () => false }); document.dispatchEvent(new Event('visibilitychange')); });
    await page.waitForTimeout(100);
    // A failed data read must not consume the revision; recovery must retry it.
    await page.evaluate(() => { window.testFailRecords = true; window.testRevision++; });
    await page.getByText('连接中断，正在重试', { exact: true }).waitFor();
    await page.evaluate(() => { window.testFailRecords = false; });
    await page.getByText('实时更新', { exact: true }).waitFor();

    const detailButton = page.locator('.cell-link').first(); await detailButton.click();
    const drawer = page.getByRole('dialog', { name: '答题详情' }); await drawer.waitFor();
    assert.ok((await drawer.locator('.detail-text').first().innerText()).length > 800);
    assert.ok((await drawer.innerText()).includes('第四个选项')); assert.ok((await drawer.innerText()).includes('模型请求超时'));
    assert.equal(await page.locator('#app').evaluate(el => el.inert), true);
    await page.keyboard.press('Shift+Tab'); assert.equal(await drawer.evaluate(el => el.contains(document.activeElement)), true);
    await page.keyboard.press('Tab'); assert.equal(await drawer.evaluate(el => el.contains(document.activeElement)), true);
    let screenshots = 0;
    for (const width of [1160, 900]) for (const theme of ['light', 'dark']) {
      await page.setViewportSize({ width, height: width === 900 ? 600 : 760 });
      await page.evaluate(theme => document.documentElement.setAttribute('data-theme', theme), theme);
      await page.waitForTimeout(220);
      assert.equal(await drawer.evaluate(el => el.scrollWidth <= el.clientWidth + 1 && el.getBoundingClientRect().right <= innerWidth), true);
      await artifacts.capture(page, `record-drawer-${theme}-${width}.png`); screenshots++;
    }
    await page.keyboard.press('Escape'); await drawer.waitFor({ state: 'detached' });
    assert.equal(await detailButton.evaluate(el => document.activeElement === el), true); assert.equal(await page.locator('#app').evaluate(el => el.inert), false);
    await page.getByRole('button', { name: '清空记录', exact: true }).click();
    const confirm = page.getByRole('dialog', { name: '清空答题记录？' }); await confirm.waitFor();
    assert.equal(await confirm.getByRole('button', { name: '取消', exact: true }).evaluate(el => document.activeElement === el), true);
    await page.keyboard.press('Escape'); assert.equal(await calls('clear_records'), 0);

    await page.goto(base + 'questions'); await page.locator('.cell-link').first().waitFor();
    await page.getByRole('button', { name: '备份 / 导出' }).click(); assert.equal(await calls('export_question_file'), 1);
    // Cancelled file picker must not show a preview.
    await page.evaluate(() => { window.testFile = null; }); await page.getByRole('button', { name: '导入 / 恢复' }).click(); await page.waitForTimeout(50); assert.equal(await page.getByRole('dialog').count(), 0);
    await page.evaluate(() => { window.testFile = ['demo.json', '[]']; }); await page.getByRole('button', { name: '导入 / 恢复' }).click();
    const preview = page.getByRole('dialog', { name: '导入预览' }); await preview.waitFor();
    await preview.getByRole('button', { name: '导入新题', exact: true }).click();
    const merge = page.getByRole('dialog', { name: '确认导入题目？' }); await merge.waitFor(); await page.keyboard.press('Escape'); await merge.waitFor({ state: 'detached' });
    assert.equal(await calls('apply_question_import'), 0); assert.equal(await preview.evaluate(el => el.contains(document.activeElement)), true);
    await preview.getByRole('radio', { name: /恢复整个题库/ }).check();
    await preview.getByRole('button', { name: '恢复题库', exact: true }).click();
    const restore = page.getByRole('dialog', { name: '恢复并替换整个题库？' }); await restore.waitFor();
    await page.waitForTimeout(250);
    await artifacts.capture(page, 'restore-confirm.png'); screenshots++;
    await restore.getByRole('button', { name: '备份并恢复', exact: true }).click(); await preview.waitFor({ state: 'detached' });
    assert.equal(await calls('apply_question_import'), 1); await page.getByText('恢复前的自动备份：', { exact: false }).waitFor();
    // Invalid content blocks writes in the UI, in addition to backend checks.
    await page.evaluate(() => { window.testPreview = { ...window.testPreview, invalid: 1, errors: ['第 1 条：答案超出范围'] }; });
    await page.getByRole('button', { name: '导入 / 恢复' }).click(); await preview.waitFor();
    assert.equal(await preview.getByRole('button', { name: '导入新题' }).isDisabled(), true);
    await artifacts.capture(page, 'invalid-import.png'); screenshots++;
    await page.keyboard.press('Escape');

    await page.goto(base + 'settings?diagnose=1'); await page.locator('.diagnostic-item').first().waitFor();
    assert.equal(await page.evaluate(() => window.testCalls.filter(c => c.cmd === 'run_diagnostics').every(c => c.args.includeModel === false)), true);
    const diagnosticCalls = await calls('run_diagnostics');
    await page.getByRole('button', { name: '模型调用检查…', exact: true }).click(); await page.keyboard.press('Escape'); assert.equal(await calls('run_diagnostics'), diagnosticCalls);
    await page.getByRole('button', { name: '模型调用检查…', exact: true }).click(); await page.getByRole('button', { name: '调用并检查', exact: true }).click();
    await page.getByText('调用正常，本次 5 Token', { exact: true }).waitFor();
    for (const width of [1160, 900]) for (const theme of ['light', 'dark']) {
      await page.setViewportSize({ width, height: width === 900 ? 600 : 760 }); await page.evaluate(theme => document.documentElement.setAttribute('data-theme', theme), theme);
      await artifacts.capture(page, `diagnostics-${theme}-${width}.png`); screenshots++;
      assert.equal(await page.locator('.content').evaluate(el => el.scrollWidth <= el.clientWidth), true);
    }
    await page.goto(base + 'records?initial-failure=1');
    await page.reload();
    await page.waitForTimeout(2300);
    assert.equal(await page.locator('.toast--err').count(), 1, 'Initial load failures must not repeatedly toast on each retry');
    await page.evaluate(() => { window.testFailRecords = false; });
    await page.locator('.cell-link').first().waitFor();
    assert.deepEqual(errors, []);
    const result = { passed: true, screenshots, liveRevisionRefresh: true, unchangedRevisionNoReload: true, hiddenPollingSuspended: true, failedRefreshRetried: true, longQuestionAndOptions: true, failureDetails: true, drawerFocusTrapAndRestore: true, inAppConfirmation: true, safeCancel: true, exportAndImportPreview: true, restoreConfirmationAndBackupPath: true, invalidImportBlocked: true, localDiagnosisNoModelCalls: true, paidDiagnosisExplicitConfirmation: true, errors, scope: 'Production Vue UI in Edge, isolated IPC fixtures; no paid model calls' };
    await fs.writeFile(path.join(output, 'features-result.json'), JSON.stringify(result, null, 2)); console.log(JSON.stringify(result));
  } finally { try { if (browser) await browser.close(); } finally { server.kill(); await artifacts.cleanup(); } }
})().catch(error => { console.error(error); process.exitCode = 1; });
