const fs = require('node:fs/promises');
const path = require('node:path');

// Keep screenshots only when deliberately requested for a visual review.
// Cleanup targets exactly the files captured by this run, never a directory.
function screenshotArtifacts(output, keep = process.env.KEEP_TEST_SCREENSHOTS === '1') {
  const generated = new Set();
  return {
    async capture(page, name) {
      if (path.basename(name) !== name || !name.endsWith('.png')) throw Error('Expected a screenshot filename');
      const target = path.join(output, name);
      generated.add(target);
      await page.screenshot({ path: target });
    },
    async cleanup() {
      if (keep) return;
      const results = await Promise.allSettled([...generated].map(target => fs.unlink(target)));
      for (const result of results) {
        if (result.status === 'rejected' && result.reason.code !== 'ENOENT') throw result.reason;
      }
    },
  };
}
module.exports = { screenshotArtifacts };
