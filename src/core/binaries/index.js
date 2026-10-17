'use strict';

const config = require('../../config');
const { probe } = require('./probe');
const { downloadToFile } = require('./downloader');
const {
    ytDlpAsset, localYtDlpPath, installYtDlp, ensureYtDlp, updateYtDlp, latestYtDlpVersion, INSTALL_DIR,
} = require('./installers/ytdlp');
const { ensureFfmpeg } = require('./installers/ffmpeg');
const { ensureCloudflared, cloudflaredAsset, localCloudflaredPath } = require('./installers/cloudflared');

module.exports = {
    ensureYtDlp, updateYtDlp, installYtDlp, latestYtDlpVersion, ensureFfmpeg,
    ensureCloudflared, cloudflaredAsset, localCloudflaredPath,
    ytDlpAsset, localYtDlpPath, probe, downloadToFile,
    BIN_DIR: config.binDir,
    INSTALL_DIR,
};
