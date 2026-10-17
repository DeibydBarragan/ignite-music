'use strict';

const { spawn } = require('node:child_process');

/**
 * Run `binary args` and resolve { ok, output, error } without throwing.
 */
function probe(binary, args = ['--version'], timeoutMs = 60_000) {
    return new Promise((resolve) => {
        let out = '';
        let err = '';
        let child;
        try {
            child = spawn(binary, args, { windowsHide: true });
        } catch (error) {
            resolve({ ok: false, error: error.message });
            return;
        }
        const timer = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
        child.stdout?.on('data', (data) => (out += data));
        child.stderr?.on('data', (data) => (err += data));
        child.on('error', (error) => {
            clearTimeout(timer);
            resolve({ ok: false, error: error.code === 'ENOENT' ? 'not found' : error.message });
        });
        child.on('close', (code) => {
            clearTimeout(timer);
            const lastLine = (err || out).trim().split(/\r?\n/).pop() || `exit code ${code}`;
            resolve(code === 0 ? { ok: true, output: out.trim() } : { ok: false, error: lastLine });
        });
    });
}

module.exports = { probe };
