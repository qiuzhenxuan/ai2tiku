// Prepare runtime secrets locally. Never prints or overwrites a secret.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const root = path.resolve(__dirname, '..', 'deploy');
const secretDir = path.join(root, 'secrets');
fs.mkdirSync(secretDir, { recursive: true, mode: 0o700 });
// Compose file-backed secrets preserve the host file's UID/mode. The private
// parent protects them on the host; read-only mounts must be readable by the
// unprivileged API container (UID 10001).
if (process.platform !== 'win32') fs.chmodSync(secretDir, 0o700);
for (const [name, value] of [
  ['master_key', crypto.randomBytes(32).toString('base64')],
  ['admin_password', crypto.randomBytes(24).toString('base64url')],
]) {
  const target = path.join(secretDir, name);
  try { fs.writeFileSync(target, value + '\n', { flag: 'wx', mode: 0o444 }); console.log(`Created ${path.relative(root, target)}`); }
  catch (error) { if (error.code !== 'EEXIST') throw error; console.log(`Kept existing ${path.relative(root, target)}`); }
  if (process.platform !== 'win32') fs.chmodSync(target, 0o444);
}
const envPath = path.join(root, '.env');
try { fs.copyFileSync(path.join(root, '.env.example'), envPath, fs.constants.COPYFILE_EXCL); console.log('Created deploy/.env; edit PUBLIC_URL and ADMIN_EMAIL before starting.'); }
catch (error) { if (error.code !== 'EEXIST') throw error; console.log('Kept existing deploy/.env'); }
console.log('Read deploy/secrets/admin_password locally to sign in. Keep master_key backed up with your data.');
