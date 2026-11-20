'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { matchVoiceCommand } = require('../src/voice/matcher');

const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'matcher-casos.json');

const CASOS_PRUEBA = [
    { text: 'Ignite, reproduce Bohemian Rhapsody!', wakeWord: 'ignite' },
    { text: 'oye ignite reproduce starboy', wakeWord: 'ignite' },
    { text: 'ignite pon', wakeWord: 'ignite' },
    { text: 'ignite', wakeWord: 'ignite' },
    { text: 'ignite un dia de estos pon musica', wakeWord: 'ignite' },
    { text: 'ayer en la night pon algo de reggaeton', wakeWord: 'ignite' },
    { text: 'ignite siguiente', wakeWord: 'ignite' },
    { text: 'ignait siguiente', wakeWord: 'ignite' },
    { text: 'ignite activa el autoplay', wakeWord: 'ignite' },
    { text: 'ignite desactiva el autoplay', wakeWord: 'ignite' },
    { text: 'ignite apaga', wakeWord: 'ignite' },
    { text: 'ignite activa el autoplay de la fiesta', wakeWord: 'ignite' },
    { text: 'Ignite, hola que hace', wakeWord: 'ignite' },
    { text: 'hola ignite que hace', wakeWord: 'ignite' },
    { text: 'Creo que se hace un poco más rápido. Bot, pausa.', wakeWord: 'bot' },
    { text: 'bot por favor puedes reproducir Starboy', wakeWord: 'bot' },
    { text: 'bot por favor pon starboy', wakeWord: 'bot' },
    { text: 'bot siguiente', wakeWord: 'ignite, bot' },
    { text: 'reproduce starboy', wakeWord: 'ignite, bot' },
    { text: 'bot pausa', wakeWord: 'ignite' },
    { text: 'bot un dia de estos pon musica', wakeWord: 'bot' },
    { text: 'ignite para hacerle la cena a mi mama', wakeWord: 'ignite' },
    {
        text: 'bot tocate Something About Us',
        wakeWord: 'bot',
        customBindings: { play: ['tocate'] },
    },
    { text: 'hola que hace', wakeWord: 'ignite' },
    { text: 'ola k ase', wakeWord: 'ignite' },
    { text: '', wakeWord: 'ignite' },
    { text: '   ', wakeWord: 'ignite' },
    { text: 'ignite anterior', wakeWord: 'ignite' },
    { text: 'ignite reanuda', wakeWord: 'ignite' },
    { text: 'ignite mezcla', wakeWord: 'ignite' },
    { text: 'ignite cola', wakeWord: 'ignite' },
    { text: 'ignite volumen al 80', wakeWord: 'ignite' },
    { text: 'ignite sube el volumen a 50', wakeWord: 'ignite' },
    { text: 'ignite saltar esta cancion por favor', wakeWord: 'ignite' },
    { text: 'ignite pausa la musica ya', wakeWord: 'ignite' },
    { text: 'ignite play next starboy', wakeWord: 'ignite' },
    { text: 'ignite añade despues cancion', wakeWord: 'ignite' },
    { text: 'ignite encola coldplay yellow', wakeWord: 'ignite' },
    { text: 'ignite apaga el autoplay', wakeWord: 'ignite' },
    { text: 'ignite canciones', wakeWord: 'ignite' },
    { text: 'ignite dale', wakeWord: 'ignite' },
    { text: 'ignite stop', wakeWord: 'ignite' },
    { text: 'ignite aleatorio', wakeWord: 'ignite' },
    { text: 'ignite random', wakeWord: 'ignite' },
];

function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    const results = CASOS_PRUEBA.map((caso) => {
        const res = matchVoiceCommand(caso.text, {
            wakeWord: caso.wakeWord,
            customBindings: caso.customBindings,
        });
        return {
            input: caso.text,
            wakeWord: caso.wakeWord,
            customBindings: caso.customBindings || null,
            coincide: res.matched,
            motivo: res.reason || null,
            palabra: res.wakeWord || null,
            comando: res.commandId || null,
            frase: res.phrase || null,
            consulta: res.query !== undefined ? res.query : null,
            faltaConsulta: res.missingQuery !== undefined ? res.missingQuery : null,
            relleno: res.filler || null,
        };
    });

    fs.writeFileSync(OUT_FILE, JSON.stringify(results, null, 2) + '\n', 'utf8');
}

main();
