'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { isLikelyHallucination } = require('../src/voice/groq-client');

const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'alucinacion-casos.json');

const TEXT_CASES = [
    '',
    '   ',
    '!!!???',
    '. , ; : - ',
    'Gracias.',
    '¡Suscríbete!',
    'Subtítulos por la comunidad de Amara.org',
    'Thank you.',
    'gracias',
    'muchas gracias',
    'gracias por ver el video',
    'gracias por ver el vídeo',
    'gracias por ver',
    'thank you',
    'thanks',
    'thanks for watching',
    'thank you for watching',
    'suscribete',
    'suscribete al canal',
    'no olvides suscribirte',
    'subtitulos por la comunidad de amara org',
    'subtitulado por la comunidad de amara org',
    'subtitulos realizados por la comunidad de amara org',
    'amara org',
    'you',
    'bye',
    'adios',
    '¡Adiós!',
    'hasta la proxima',
    'Hasta la próxima...',
    'ignite pausa',
    'ignite pon algo',
    'reproduce bad bunny',
    'oye ignite siguiente cancion',
    'sube el volumen al 50',
];

const SEGMENT_COMBINATIONS = [
    { label: 'none', segments: [] },
    { label: 'one_silent', segments: [{ no_speech_prob: 0.9, avg_logprob: -1.5 }] },
    { label: 'one_borderline', segments: [{ no_speech_prob: 0.6, avg_logprob: -1.0 }] },
    { label: 'one_real', segments: [{ no_speech_prob: 0.05, avg_logprob: -0.2 }] },
    {
        label: 'two_silent',
        segments: [
            { no_speech_prob: 0.85, avg_logprob: -1.2 },
            { no_speech_prob: 0.95, avg_logprob: -2.0 },
        ],
    },
    {
        label: 'silent_and_real',
        segments: [
            { no_speech_prob: 0.9, avg_logprob: -1.5 },
            { no_speech_prob: 0.05, avg_logprob: -0.2 },
        ],
    },
    { label: 'empty_segment_obj', segments: [{}] },
    { label: 'only_no_speech', segments: [{ no_speech_prob: 0.95 }] },
    { label: 'only_avg_logprob', segments: [{ avg_logprob: -2.5 }] },
];

function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    const results = [];

    for (const text of TEXT_CASES) {
        for (const seg of SEGMENT_COMBINATIONS) {
            const esAlucinacion = isLikelyHallucination(text, seg.segments);
            results.push({
                texto: text,
                etiquetaSegmentos: seg.label,
                segmentos: seg.segments,
                esAlucinacion,
            });
        }
    }

    fs.writeFileSync(OUT_FILE, JSON.stringify(results, null, 2) + '\n', 'utf8');
}

main();
