'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { VoiceSettingsStore } = require('../src/voice/settings-store');
const { schemas } = require('../src/api/middleware/validate');
const { register } = require('../src/api/v1/ops/voice');

test('VoiceSettingsStore: returns defaults and manages in-memory cache', async () => {
    const store = new VoiceSettingsStore();

    // Default settings
    const defaults = store.getDefaultSettings();
    assert.equal(defaults.wake_word, 'ignite');
    assert.equal(typeof defaults.tts_templates, 'object');
    assert.equal(defaults.tts_templates.play, 'Reproduciendo {query}');
    assert.deepEqual(defaults.bindings, []);

    // Guild without database record returns defaults
    const guildSettings = await store.getGuildSettings('guild_test_1');
    assert.equal(guildSettings.wake_word, 'ignite');

    // Update settings in memory
    const updated = await store.updateGuildSettings('guild_test_1', {
        wake_word: 'botcito',
        tts_feedback_enabled: true,
        bindings: [
            { phrase: 'metele candela', command_id: 'play' },
        ],
    });

    assert.equal(updated.wake_word, 'botcito');
    assert.equal(updated.tts_feedback_enabled, true);
    assert.equal(updated.bindings.length, 1);
    assert.equal(updated.bindings[0].phrase, 'metele candela');

    // Cached retrieval returns updated values
    const cached = await store.getGuildSettings('guild_test_1');
    assert.equal(cached.wake_word, 'botcito');
    assert.equal(cached.bindings[0].phrase, 'metele candela');

    // Reset settings
    const reset = await store.resetGuildSettings('guild_test_1');
    assert.equal(reset.wake_word, 'ignite');
    assert.deepEqual(reset.bindings, []);
});

test('voice.routes Zod schema: validates voiceUpdate body payload', () => {
    const schema = schemas.body.voiceUpdate;

    // Valid payload
    const valid = schema.safeParse({
        enabled: true,
        wake_word: 'asistente',
        tts_feedback_enabled: true,
        tts_templates: {
            play: 'Poniendo {query}',
        },
        bindings: [
            { phrase: 'ponte esta', command_id: 'play' },
            { phrase: 'saltala', command_id: 'skip' },
        ],
    });
    assert.equal(valid.success, true);
    assert.equal(valid.data.wake_word, 'asistente');

    // Invalid bindings format
    const invalidBinding = schema.safeParse({
        bindings: [{ phrase: '', command_id: 'play' }],
    });
    assert.equal(invalidBinding.success, false);

    // Empty wake word rejected
    const invalidWake = schema.safeParse({
        wake_word: '',
    });
    assert.equal(invalidWake.success, false);
});

test('voice ops: registers and delegates voice.get, voice.update, voice.reset', async () => {
    const registry = new Map();
    const mockOps = {
        register: (name, fn) => registry.set(name, fn),
    };

    register(mockOps);
    assert.ok(registry.has('voice.get'));
    assert.ok(registry.has('voice.update'));
    assert.ok(registry.has('voice.reset'));

    // Execute get op
    const getFn = registry.get('voice.get');
    const settings = await getFn(null, { guildId: 'g_ops_test' });
    assert.equal(settings.wake_word, 'ignite');
});
