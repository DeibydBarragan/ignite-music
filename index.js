'use strict';

// Start the bot with any of: npm start, node ., node index.js, start.bat, start-shard.bat.
// 1) Node.js version and npm packages (installed automatically, no npm package needed for this step)
const { SetupError } = require('./src/setup/setup-error');
try {
    require('./src/setup/bootstrap').run();
} catch (error) {
    // report.js already printed what to do; anything else is a real bug.
    if (!(error instanceof SetupError)) throw error;
    process.exit(1);
}
// 2) Resolver hardening (needs dotenv from node_modules, so it runs after step 1)
require('./src/core/dns-fix');
// 3) Configuration, Discord token, yt-dlp, ffmpeg, then the sharding manager
require('./src/setup/launcher').main();
