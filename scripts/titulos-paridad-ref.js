'use strict';

const fs = require('node:fs');
const path = require('node:path');
const {
    splitYouTubeTitle,
    parseYouTubeUrl,
    streamTtl,
    isLyricTitle,
} = require('../src/music/plugins-base');

const targetPath = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'medios',
    'tests',
    'data',
    'titulos-casos.json',
);

const TITLE_CASES = [
    { title: 'Xavi - La Diabla (Official Video)', uploader: 'Xavi Oficial' },
    { title: 'La Diabla', uploader: 'Xavi - Topic' },
    { title: 'lofi hip hop radio - beats to relax/study to 📚', uploader: 'Lofi Girl' },
    { title: 'Shakira - BZRP Music Sessions #53', uploader: 'Shakira' },
    { title: 'Queen - Bohemian Rhapsody (Official Video)', uploader: 'QueenVEVO' },
    { title: 'Queen - Don\'t Stop Me Now [Official Video]', uploader: 'Queen' },
    { title: 'Feid - LUNA (Audio Oficial)', uploader: 'Feid' },
    { title: 'Bad Bunny - Monaco (Video Oficial)', uploader: 'Bad Bunny' },
    { title: 'Xavi - Diablita (Official Lyric Video)', uploader: 'Xavi Official' },
    { title: 'Coldplay - Yellow (Lyrics)', uploader: 'Coldplay' },
    { title: 'Dua Lipa - Houdini (Visualizer)', uploader: 'Dua Lipa' },
    { title: 'Alone Without Dash Here', uploader: 'SomeArtist' },
    { title: 'Artist - Song With Multiple - Dashes in Title', uploader: 'Artist' },
    { title: 'Unknown Channel Song', uploader: '' },
    { title: 'Topic Without Prefix - Topic', uploader: 'Topic' },
    { title: 'Artista Con Acentos - Canción (Audio Oficial)', uploader: 'Artista Con Acentos' },
];

const URL_CASES = [
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
    'https://youtu.be/dQw4w9WgXcQ?si=abc',
    'https://m.youtube.com/watch?v=dQw4w9WgXcQ&t=10',
    'https://music.youtube.com/watch?v=dQw4w9WgXcQ',
    'https://www.youtube.com/shorts/dQw4w9WgXcQ',
    'https://www.youtube.com/live/dQw4w9WgXcQ',
    'https://www.youtube.com/embed/dQw4w9WgXcQ',
    'https://www.youtube.com/v/dQw4w9WgXcQ',
    'https://www.youtube.com/playlist?list=PL1234567890',
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL1234567890',
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=RDdQw4w9WgXcQ',
    'https://www.youtube.com/channel/abc',
    'https://soundcloud.com/a/b',
    'not a url',
    'https://www.youtube.com/watch?v=short',
    'https://youtube-nocookie.com/embed/dQw4w9WgXcQ',
    'https://www.youtu.be/dQw4w9WgXcQ',
];

const TTL_CASES = [
    'https://rr1.googlevideo.com/videoplayback?id=123&expire=1800000000',
    'https://example.com/audio.mp3',
    'https://not-a-valid-url-at-all',
    'https://rr1.googlevideo.com/videoplayback?expire=invalid',
];

const titleResults = TITLE_CASES.map((c) => {
    const split = splitYouTubeTitle(c.title, c.uploader);
    return {
        rawTitle: c.title,
        uploader: c.uploader,
        expectedTitle: split.title,
        expectedArtist: split.artist,
        isLyric: isLyricTitle(c.title),
    };
});

const urlResults = URL_CASES.map((u) => ({
    input: u,
    expected: parseYouTubeUrl(u),
}));

const ttlResults = TTL_CASES.map((u) => ({
    input: u,
    expected: streamTtl(u),
}));

const output = {
    titles: titleResults,
    urls: urlResults,
    ttls: ttlResults,
};

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, JSON.stringify(output, null, 2) + '\n', 'utf8');
console.log(`Generated titulos parity reference at ${targetPath}`);
