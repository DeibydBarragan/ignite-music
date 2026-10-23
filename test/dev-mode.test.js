'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { isDevMode, setDevMode, getDevUsers } = require('../src/core/dev-mode');
const { recordPlay, historyStore } = require('../src/music/history');

test('DevMode core manager: enables and disables user IDs', () => {
    const testUid = '999888777666555444';
    assert.equal(isDevMode(testUid), false);

    setDevMode(testUid, true);
    assert.equal(isDevMode(testUid), true);
    assert.ok(getDevUsers().includes(testUid));

    setDevMode(testUid, false);
    assert.equal(isDevMode(testUid), false);
    assert.ok(!getDevUsers().includes(testUid));
});

test('recordPlay skips recording when devMode is active for user', async () => {
    const devUid = '111222333444555666';
    setDevMode(devUid, true);

    const initialHistory = historyStore.get('test-guild-dev') || [];
    const initialCount = initialHistory.length;

    const fakeSong = {
        id: 'dev-song-1',
        name: 'Test Dev Song',
        uploader: { name: 'Dev Artist' },
        user: { id: devUid, username: 'DevTester' },
        metadata: { requesterId: devUid },
    };

    await recordPlay('test-guild-dev', fakeSong);

    const afterHistory = historyStore.get('test-guild-dev') || [];
    assert.equal(afterHistory.length, initialCount, 'Play should not be recorded in history when user is in dev mode');

    setDevMode(devUid, false);
});

test('recordPlay skips recording when song.metadata.devMode is true', async () => {
    const normalUid = '222333444555666777';
    setDevMode(normalUid, false);

    const initialHistory = historyStore.get('test-guild-dev-meta') || [];
    const initialCount = initialHistory.length;

    const fakeSong = {
        id: 'dev-song-2',
        name: 'Test Dev Song Explicit',
        user: { id: normalUid, username: 'NormalTester' },
        metadata: { requesterId: normalUid, devMode: true },
    };

    await recordPlay('test-guild-dev-meta', fakeSong);

    const afterHistory = historyStore.get('test-guild-dev-meta') || [];
    assert.equal(afterHistory.length, initialCount, 'Play should not be recorded when metadata.devMode is true');
});
