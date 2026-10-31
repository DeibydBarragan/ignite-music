'use strict';

const fs = require('node:fs');
const path = require('node:path');

function patchVoice() {
    try {
        const filePath = path.join(__dirname, '..', 'node_modules', '@discordjs', 'voice', 'dist', 'index.js');
        if (!fs.existsSync(filePath)) return;

        let content = fs.readFileSync(filePath, 'utf8');

        // 1. IP discovery packet retry
        if (!content.includes('[UDP] retrying IP discovery packet')) {
            const target = `      this.send(discoveryBuffer);\n    });`;
            const targetCRLF = `      this.send(discoveryBuffer);\r\n    });`;

            const replacement = `      this.send(discoveryBuffer);
      const retry = setInterval(() => {
        try {
          this.emit('debug', '[UDP] retrying IP discovery packet to ' + this.remote?.ip + ':' + this.remote?.port);
          this.send(discoveryBuffer);
        } catch {}
      }, 500);
      const origResolve = resolve2;
      resolve2 = (pkt) => { clearInterval(retry); origResolve(pkt); };
      const origReject = reject;
      reject = (err) => { clearInterval(retry); origReject(err); };
    });`;

            if (content.includes(target)) {
                content = content.replace(target, replacement);
            } else if (content.includes(targetCRLF)) {
                content = content.replace(targetCRLF, replacement.replace(/\n/g, '\r\n'));
            }
        }

        // 2. Prevent 4006 Session Invalidation infinite reconnect loop
        const closeTarget = `    if (code === 4014) {`;
        const closeReplacement = `    if (code === 4014 || code === 4006) {`;
        if (content.includes(closeTarget)) {
            content = content.replace(closeTarget, closeReplacement);
        }

        fs.writeFileSync(filePath, content, 'utf8');
    } catch {}
}

patchVoice();

module.exports = patchVoice;
