// Real Rust API + production Vue UI; only provider model metadata is mocked.
// Isolated accounts/data, no model requests,
// screenshots, desktop windows, native dialogs, or persisted test credentials.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const previewPort = Number(process.env.HOSTED_TEST_PORT || 4185);
const publicUrl = 'http://127.0.0.1:' + previewPort;
async function main() {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), 'ai2tiku-hosted-e2e-'));
  const password = crypto.randomBytes(24).toString('hex');
  const key = 'sk-e2e-not-real-' + crypto.randomBytes(16).toString('hex');
  const children = [];
  let browser;
  async function stopChildren() {
    await Promise.all(children.map(child => new Promise(resolve => {
      if (child.exitCode !== null) return resolve();
      child.once('exit', resolve); child.kill();
      setTimeout(resolve, 3000).unref();
    })));
  }
  try {
    const binary = process.env.HOSTED_BINARY || path.join(root, 'src-tauri', 'target', 'debug', process.platform === 'win32' ? 'ai2tiku-server.exe' : 'ai2tiku-server');
    const environment = {
      ...process.env, PUBLIC_URL: publicUrl, HOSTED_BIND: '127.0.0.1:8080', DATA_DIR: dataDir,
      ADMIN_EMAIL: 'owner@example.com', ADMIN_PASSWORD: password,
      MASTER_KEY: crypto.randomBytes(32).toString('base64'), REGISTRATION_OPEN: 'false',
    };
    delete environment.MASTER_KEY_FILE; delete environment.ADMIN_PASSWORD_FILE;
    delete environment.ALLOWED_PROVIDER_URLS;
    children.push(spawn(binary, [], { cwd: root, windowsHide: true, stdio: 'ignore', env: environment }));
    children[0].on('error', error => { throw error; });
    const wait = async url => {
      for (let i = 0; i < 100; i++) {
        try { if ((await fetch(url)).ok) return; } catch {}
        if (children.some(c => c.exitCode !== null)) throw Error('A test service exited before becoming ready');
        await new Promise(resolve => setTimeout(resolve, 200));
      }
      throw Error('Timed out waiting for isolated test service');
    };
    await wait('http://127.0.0.1:8080/healthz');
    children.push(spawn(process.execPath, [path.join(root, 'node_modules/vite/bin/vite.js'), 'preview', '--config', 'web/vite.config.ts', '--host', '127.0.0.1', '--port', String(previewPort), '--strictPort'], { cwd: root, windowsHide: true, stdio: 'ignore' }));
    await wait(publicUrl);
    for (const method of ['HEAD', 'GET']) {
      const probe = await fetch(publicUrl + '/api/search?t=' + Date.now(), { method });
      assert.equal(probe.status, 200, 'OCS unauthenticated connectivity probes must succeed');
      if (method === 'HEAD') assert.equal(await probe.text(), '');
      else assert.equal((await probe.json()).code, 0, 'Connectivity probe must not return an answer');
    }
    const api = async (method, endpoint, body, cookie, origin = publicUrl) => {
      const response = await fetch('http://127.0.0.1:8080' + endpoint, { method, headers: {
        ...(origin ? { Origin: origin } : {}), ...(cookie ? { Cookie: cookie } : {}),
        ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
      }, body: body === undefined ? undefined : JSON.stringify(body) });
      return { status: response.status, data: await response.json(), cookie: response.headers.get('set-cookie')?.split(';')[0] };
    };
    const login = await api('POST', '/api/console/auth/login', { email: 'owner@example.com', password });
    assert.equal(login.status, 200); const admin = login.cookie;
    const create = async email => (await api('POST', '/api/console/admin/users', { email, password, dailyLimit: 3 }, admin)).data;
    const userA = await create('a@example.com'), userB = await create('b@example.com');
    assert.ok(userA.id && userB.id);
    const sessionA = (await api('POST', '/api/console/auth/login', { email: 'a@example.com', password })).cookie;
    const sessionB = (await api('POST', '/api/console/auth/login', { email: 'b@example.com', password })).cookie;
    assert.equal((await api('GET', '/api/console/me')).status, 401);
    assert.equal((await api('GET', '/api/console/admin/users', undefined, sessionA)).status, 403);
    assert.equal((await api('POST', '/api/console/tokens', { name: 'blocked' }, sessionA, 'https://other.invalid')).status, 403);
    assert.equal((await api('POST', '/api/console/auth/register', { email: 'c@example.com', password })).status, 403);

    browser = await chromium.launch({ headless: true, ...(process.env.EDGE_PATH ? { executablePath: process.env.EDGE_PATH } : {}) });
    const context = await browser.newContext({ viewport: { width: 1280, height: 850 } });
    await context.addCookies([{ name: 'ai2tiku_session', value: sessionA.split('=')[1], url: publicUrl }]);
    const page = await context.newPage(), errors = []; page.on('pageerror', error => errors.push(error.message));
    let discoveryRequests = 0;
    await page.route('**/api/console/models/discover', async route => {
      const input = route.request().postDataJSON();
      assert.equal(input.providerId, 'deepseek');
      assert.ok(input.apiKey === key || input.apiKey === undefined);
      discoveryRequests++;
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ models: ['deepseek-chat', 'deepseek-reasoner'] }) });
    });
    await page.goto(publicUrl); await page.getByRole('navigation', { name: '主导航' }).waitFor();
    const github = page.getByRole('link', { name: /GitHub/ }).first();
    assert.equal(await github.getAttribute('href'), 'https://github.com/qiuzhenxuan/ai2tiku'); assert.equal(await github.getAttribute('target'), '_blank');
    await page.getByRole('button', { name: /模型与接入/ }).click();
    await page.getByLabel('模型供应商').selectOption('deepseek');
    await page.getByLabel('模型 API Key', { exact: true }).fill(key);
    await page.getByLabel('可用模型', { exact: true }).waitFor();
    await page.getByLabel('可用模型', { exact: true }).selectOption('deepseek-reasoner');
    assert.equal(discoveryRequests, 1, 'Typing a Key automatically retrieves the provider model list');
    const savedResponse = page.waitForResponse(response => response.url().endsWith('/api/console/model') && response.request().method() === 'PUT');
    await page.getByRole('button', { name: '保存模型配置', exact: true }).click();
    const saved = await savedResponse;
    assert.equal(saved.status(), 200, `Saving the selected model failed: ${await saved.text()}`);
    await page.getByText('Key 已配置', { exact: true }).waitFor();
    assert.equal(await page.getByLabel('模型 API Key', { exact: true }).inputValue(), '');
    const storedModel = (await api('GET', '/api/console/model', undefined, sessionA)).data;
    assert.equal(storedModel.model, 'deepseek-reasoner');
    await page.getByLabel('令牌名称', { exact: true }).fill('测试 OCS');
    await page.getByRole('button', { name: /生成令牌与 OCS 配置/ }).click();
    const dialog = page.getByRole('dialog', { name: '访问令牌已生成' }); await dialog.waitFor();
    const token = await dialog.getByLabel('题库访问令牌', { exact: true }).inputValue();
    const config = JSON.parse(await dialog.getByLabel('OCS 题库配置', { exact: true }).inputValue());
    assert.equal(config[0].headers.Authorization, 'Bearer ' + token); assert.equal(config[0].url, publicUrl + '/api/search');
    assert.ok(!JSON.stringify(config).includes(key));
    await page.keyboard.press('Escape'); assert.equal(await dialog.count(), 1, 'Secret modal requires explicit acknowledgement');
    await dialog.getByRole('checkbox').check(); await dialog.getByRole('button', { name: '完成并关闭', exact: true }).click();
    assert.equal(await page.getByRole('dialog').count(), 0);
    await page.reload(); await page.getByRole('navigation', { name: '主导航' }).waitFor();
    assert.equal(await page.locator('body').evaluate((el, token) => el.textContent.includes(token), token), false);
    const listed = await api('GET', '/api/console/tokens', undefined, sessionA);
    assert.ok(!JSON.stringify(listed.data).includes(token)); assert.equal(listed.data.items[0].prefix, token.slice(0, listed.data.items[0].prefix.length));
    assert.equal((await api('DELETE', '/api/console/tokens/' + listed.data.items[0].id, undefined, sessionB)).status, 404);
    const search = async bearer => {
      const response = await fetch('http://127.0.0.1:8080/api/search', { method: 'POST', headers: { Authorization: 'Bearer ' + bearer, 'Content-Type': 'application/json' }, body: JSON.stringify({ title: '', type: 'single', options: '' }) });
      return { status: response.status, data: await response.json() };
    };
    assert.equal((await search(token)).data.code, 0); // Empty title: never sends a model request.
    assert.equal((await api('GET', '/api/console/records', undefined, sessionA)).data.total, 1);
    assert.equal((await api('GET', '/api/console/records', undefined, sessionB)).data.total, 0);
    await search(token); await search(token); assert.equal((await search(token)).status, 429);
    assert.equal((await api('PATCH', '/api/console/admin/users/' + userA.id, { dailyLimit: 0 }, admin)).status, 200);
    assert.equal((await search(token)).status, 200, 'Unlimited users can continue after exhausting a prior finite cap');
    const unlimitedUser = (await api('GET', '/api/console/me', undefined, sessionA)).data;
    assert.equal(unlimitedUser.dailyLimit, 0); assert.equal(unlimitedUser.usedToday, 4, 'Unlimited keeps request statistics');
    await page.reload(); await page.getByRole('navigation', { name: '主导航' }).waitFor();
    await page.getByText('/ 无上限', { exact: true }).waitFor();
    assert.equal(await page.getByRole('progressbar').count(), 0, 'Unlimited quota has no zero-max progress bar');
    await api('DELETE', '/api/console/admin/users/' + userA.id + '/tokens', undefined, admin);
    assert.equal((await search(token)).status, 401);
    assert.equal((await api('GET', '/api/console/me', undefined, sessionA)).status, 200);
    await api('PATCH', '/api/console/admin/users/' + userA.id, { enabled: false }, admin);
    assert.equal((await api('GET', '/api/console/me', undefined, sessionA)).status, 401);
    const audit = (await api('GET', '/api/console/admin/audit', undefined, admin)).data;
    assert.ok(!JSON.stringify(audit).includes(key) && !JSON.stringify(audit).includes(token));
    await page.reload(); await page.getByLabel('邮箱', { exact: true }).waitFor();
    assert.equal(await page.getByRole('link', { name: /GitHub/ }).first().getAttribute('href'), 'https://github.com/qiuzhenxuan/ai2tiku');
    await page.getByLabel('邮箱', { exact: true }).fill('owner@example.com'); await page.getByLabel('密码', { exact: true }).fill(password);
    await page.getByRole('button', { name: '进入工作台 →', exact: true }).click(); await page.getByRole('button', { name: /用户管理/ }).waitFor();
    await page.getByRole('button', { name: /用户管理/ }).click(); await page.getByText('b@example.com', { exact: true }).waitFor();
    for (const width of [1280, 900, 390]) {
      await page.setViewportSize({ width, height: 850 }); await page.waitForTimeout(100);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'Responsive page must not overflow');
    }
    await page.emulateMedia({ reducedMotion: 'reduce' });
    assert.deepEqual(errors, []);
    // Check secret storage while the process is live, including WAL contents.
    for (const name of await fs.readdir(dataDir)) {
      if (!name.startsWith('hosted.sqlite3')) continue;
      const bytes = await fs.readFile(path.join(dataDir, name));
      assert.equal(bytes.includes(Buffer.from(key)), false); assert.equal(bytes.includes(Buffer.from(token)), false);
    }
    const result = { passed: true, realBackend: true, csrf: true, tenantIsolation: true, roleChecks: true, oneTimeToken: true, encryptedModelKey: true, automaticModelDiscoveryUI: true, providerMetadataMocked: true, ocsHeadGetProbe: true, quotaEnforced: true, unlimitedQuota: true, unlimitedUsageCounted: true, unlimitedUI: true, adminRevoke: true, disabledSessions: true, githubLinks: true, responsiveWidths: [1280, 900, 390], screenshots: 0, modelCalls: 0, errors };
    await fs.mkdir(path.join(root, 'test-results/hosted'), { recursive: true }); await fs.writeFile(path.join(root, 'test-results/hosted/result.json'), JSON.stringify(result, null, 2)); console.log(JSON.stringify(result));
  } finally {
    if (browser) await browser.close(); await stopChildren();
    // Only this script's unique temporary data directory, never a user data path.
    if (path.dirname(dataDir) !== os.tmpdir() || !path.basename(dataDir).startsWith('ai2tiku-hosted-e2e-')) throw Error('Unexpected test directory');
    await fs.rm(dataDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
