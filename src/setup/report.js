'use strict';

/**
 * Startup checklist output. Uses only Node built-ins and reads the environment
 * directly: bootstrap.js runs before npm packages (and therefore dotenv) exist,
 * so this module must never require config.js.
 */
const { SetupError } = require('./setup-error');

const color = process.stdout.isTTY && !process.env.NO_COLOR;
const paint = (code, text) => (color ? `\x1b[${code}m${text}\x1b[0m` : text);

const report = {
    title(text) {
        console.log(`\n${paint('1', text)}\n`);
    },
    ok(label, detail = '') {
        console.log(`  ${paint('32', '[ OK ]')} ${label}${detail ? paint('90', `  ${detail}`) : ''}`);
    },
    info(label, detail = '') {
        console.log(`  ${paint('36', '[INFO]')} ${label}${detail ? paint('90', `  ${detail}`) : ''}`);
    },
    // Warnings and failures go to stderr so startup errors survive even when
    // stdout is piped or redirected.
    warn(label, lines = []) {
        console.error(`  ${paint('33', '[WARN]')} ${label}`);
        for (const line of [].concat(lines)) console.error(`         ${line}`);
    },
    fail(label, lines = []) {
        console.error(`  ${paint('31', '[FAIL]')} ${paint('1', label)}`);
        for (const line of [].concat(lines)) console.error(`         ${line}`);
    },
    /**
     * Print why the bot cannot start and how to fix it, then throw SetupError.
     * Entry points (index.js, launcher.js) decide to exit, so nothing inside
     * the setup modules can kill the process behind their back.
     */
    stop(problems) {
        console.error(`\n${paint('31;1', `The bot was not started: ${problems} problem(s) above must be fixed first.`)}`);
        console.error('Fix them and start the bot again. Every step is also explained in README.md.\n');
        throw new SetupError(problems);
    },
};

module.exports = report;
