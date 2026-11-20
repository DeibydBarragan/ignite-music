'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { isDevMode, isDevUser, setDevMode, getDevUsers } = require('../src/core/dev-mode');
const { checkVoice } = require('../src/handlers/guards');
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

test('isDevUser recognizes configured developer user IDs', () => {
    assert.equal(isDevUser('472144161357824000'), true);
    assert.equal(isDevUser('123456789012345678'), false);
    assert.equal(isDevUser(null), false);
    assert.equal(isDevUser(''), false);
});

test('checkVoice: only dev user can play without being in a voice channel', () => {
    const t = (k) => k;
    const mockManager = {
        getQueue: () => null,
    };
    const mockBotChannel = {
        id: 'bot-vc-1',
        name: 'General Voice',
        isVoiceBased: () => true,
        permissionsFor: () => ({ has: () => true }),
        full: false,
    };

    // 1. Normal user not in voice channel -> rejected
    const normalInteraction = {
        user: { id: '999999999999999999' },
        guild: {
            id: 'guild-1',
            members: {
                me: { voice: { channel: mockBotChannel, channelId: mockBotChannel.id } },
            },
            channels: { cache: new Map() },
        },
        member: { voice: { channel: null } },
    };
    const normalResult = checkVoice(normalInteraction, mockManager, t, { requireQueue: false, join: true });
    assert.ok(normalResult.error, 'Normal user should get voice_channel_required error');
    assert.equal(normalResult.error, 'commands.play.voice_channel_required');

    // 2. Dev user not in voice channel -> allowed, uses bot channel
    const devInteraction = {
        user: { id: '472144161357824000' },
        guild: {
            id: 'guild-1',
            members: {
                me: { voice: { channel: mockBotChannel, channelId: mockBotChannel.id } },
            },
            channels: { cache: new Map() },
        },
        member: { voice: { channel: null } },
    };
    const devResult = checkVoice(devInteraction, mockManager, t, { requireQueue: false, join: true });
    assert.equal(devResult.error, undefined, 'Dev user should not get voice error');
    assert.equal(devResult.channel.id, mockBotChannel.id);
});
