'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { SessionStore } = require('../src/music/session');

const plugins = {
    youtube: { type: 'extractor', name: 'yt' },
    spotify: { type: 'info-extractor', name: 'sp' },
};

const defaultTargetDir = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'player',
    'tests',
    'data',
    'sesiones',
);

function makeFakeMember(id = 'user_111', username = 'Alice', avatar = 'avatar1.png', bot = false) {
    return {
        id,
        user: { id, username, avatar, bot },
        displayAvatarURL: () => avatar,
    };
}

function makeFakeSong(plugin, id, name, url, duration, member, uploaderName, isBot = false) {
    return {
        plugin,
        source: plugin === plugins.youtube ? 'youtube' : 'spotify',
        stream: { playFromSource: plugin === plugins.youtube },
        id,
        name,
        url,
        duration,
        isLive: false,
        thumbnail: `https://i.ytimg.com/vi/${id}/hqdefault.jpg`,
        uploader: { name: uploaderName, url: `https://youtube.com/@${uploaderName.toLowerCase().replace(/\s+/g, '')}` },
        ageRestricted: false,
        member,
        metadata: { requesterId: member?.id, requesterUser: member?.user },
        autoplay: isBot,
    };
}

async function escribir(targetDir) {
    fs.mkdirSync(targetDir, { recursive: true });
    const store = new SessionStore(plugins, targetDir);
    await store.store.initialize();

    const m1 = makeFakeMember('user_111', 'Alice', 'avatar1.png', false);
    const m2 = makeFakeMember('user_222', 'Bob', null, false);
    const botMember = makeFakeMember('bot_999', 'MusicBot', null, true);

    const s1 = makeFakeSong(plugins.youtube, 'vid0001', 'Song Title 1', 'https://www.youtube.com/watch?v=vid0001', 210, m1, 'Artist 1', false);
    const s2 = makeFakeSong(plugins.spotify, 'vid0002', 'Song Title 2', 'https://www.youtube.com/watch?v=vid0002', 220, m2, 'Artist 2', false);
    const s3 = makeFakeSong(plugins.youtube, 'vid0003', 'Song Title 3', 'https://www.youtube.com/watch?v=vid0003', 230, botMember, 'Artist 3', true);
    const sUnknown = makeFakeSong({ type: 'unknown_plugin' }, 'vid0004', 'Unknown Song', 'https://example.com/4', 150, m1, 'Artist X', false);

    const qNormal = {
        id: 'guild_normal',
        songs: [s1, s2, s3],
        voiceChannel: { id: 'voice_channel_1' },
        textChannel: { id: 'text_channel_1' },
        currentTime: 45,
        volume: 75,
        repeatMode: 1,
        autoplay: true,
        paused: false,
        filters: { names: ['bassboost', 'nightcore'] },
    };
    await store.save(qNormal);

    const qEmpty = {
        id: 'guild_empty_songs',
        songs: [],
        voiceChannel: { id: 'voice_channel_1' },
        textChannel: { id: 'text_channel_1' },
        currentTime: 45,
        volume: 75,
        repeatMode: 1,
        autoplay: true,
        paused: false,
        filters: { names: ['bassboost', 'nightcore'] },
    };
    // Force write empty file for fixture testing
    const emptyFile = path.join(targetDir, 'guild_empty_songs.json');
    fs.writeFileSync(emptyFile, JSON.stringify({
        voiceChannelId: qEmpty.voiceChannel.id,
        textChannelId: qEmpty.textChannel.id,
        songs: [],
        position: 45,
        volume: 75,
        repeatMode: 1,
        autoplay: true,
        filters: ['bassboost', 'nightcore'],
        paused: false,
        savedAt: Date.now(),
    }, null, 2));

    const qUnknown = {
        id: 'guild_unknown_plugin',
        songs: [s1, sUnknown],
        voiceChannel: { id: 'voice_channel_1' },
        textChannel: { id: 'text_channel_1' },
        currentTime: 45,
        volume: 75,
        repeatMode: 1,
        autoplay: true,
        paused: false,
        filters: { names: ['bassboost', 'nightcore'] },
    };
    await store.save(qUnknown);

    const expiredFile = path.join(targetDir, 'guild_expired.json');
    const expiredData = { ...store.store.get('guild_normal') };
    expiredData.savedAt = Date.now() - (35 * 60 * 1000);
    fs.writeFileSync(expiredFile, JSON.stringify(expiredData, null, 2));

    await store.drain();
    console.log(`Sesiones de paridad generadas en: ${targetDir}`);
}

async function leer(targetDir) {
    const store = new SessionStore(plugins, targetDir);
    await store.store.initialize();
    const keys = store.guildIds().sort();
    const values = {};
    for (const key of keys) {
        values[key] = store.store.get(key);
    }
    console.log(JSON.stringify({ keys, values }));
}

async function main() {
    const args = process.argv.slice(2);
    if (args[0] === '--leer') {
        const targetDir = args[1];
        if (!targetDir) {
            console.error('Error: se requiere la ruta para --leer');
            process.exit(1);
        }
        await leer(targetDir);
    } else {
        const targetDir = args[0] || defaultTargetDir;
        await escribir(targetDir);
    }
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
