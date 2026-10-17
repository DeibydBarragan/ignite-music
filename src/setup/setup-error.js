'use strict';

/**
 * Thrown by the startup checks (bootstrap, preflight) when the bot cannot run
 * until the user fixes something by hand. The human-readable explanation has
 * already been printed by report.js; entry points only need to exit.
 *
 * Lives here instead of report.js so bootstrap.js can require it before
 * npm packages (and therefore config.js) exist.
 */
class SetupError extends Error {
    constructor(problems = 1) {
        super(`The bot was not started: ${problems} problem(s) must be fixed first.`);
        this.name = 'SetupError';
        this.problems = problems;
    }
}

module.exports = { SetupError };
