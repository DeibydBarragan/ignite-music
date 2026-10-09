'use strict';

const { Playlist } = require('distube');
const { command } = require('./_shared');
const { checkVoice, panelChannel, say } = require('../handlers/guards');
const { fetchUserPlaylists } = require('../music/playlists');
const views = require('../ui/views');
const log = require('../core/logger').createLogger('playlists-cmd');

function findPlaylist(playlists, query) {
    const q = String(query || '').trim().toLowerCase();
    if (!q) return null;
    return (
        playlists.find((pl) => String(pl.url).toLowerCase() === q) ||
        playlists.find((pl) => String(pl.name).toLowerCase() === q) ||
        playlists.find((pl) => String(pl.name).toLowerCase().includes(q)) ||
        null
    );
}

module.exports = {
    category: 'playback',
    data: command('playlists', 'Ver tus playlists guardadas y reproducirlas')
        .addStringOption((option) => option
            .setName('nombre')
            .setDescription('Filtrar por nombre de playlist')
            .setMaxLength(60))
        .addStringOption((option) => option
            .setName('reproducir')
            .setDescription('Nombre o enlace de tu playlist para sonarla ahora')
            .setMaxLength(500))
        .addIntegerOption((option) => option
            .setName('page')
            .setDescription('Número de página')
            .setMinValue(1)),

    async execute(interaction, { t, manager }) {
        await interaction.deferReply({ flags: 'Ephemeral' });
        const userId = String(interaction.user.id);
        const filter = (interaction.options.getString('nombre') || '').trim();
        const toPlay = (interaction.options.getString('reproducir') || '').trim();
        const page = Math.max(0, (interaction.options.getInteger('page') ?? 1) - 1);

        let playlists;
        try {
            playlists = await fetchUserPlaylists(userId);
        } catch (error) {
            log.error(`Failed to fetch playlists for ${userId}: ${error.message}`);
            return say(interaction, 'No pude cargar tus playlists ahora mismo. Inténtalo de nuevo en un momento.');
        }

        // ─── Reproducir una playlist guardada ───
        if (toPlay) {
            const match = findPlaylist(playlists, toPlay);
            if (!match) {
                const names = playlists.slice(0, 5).map((pl) => `• ${pl.name}`).join('\n');
                return say(interaction, names
                    ? `No encontré ninguna playlist con "${toPlay}".\nTus playlists:\n${names}`
                    : `No encontré ninguna playlist con "${toPlay}". Aún no tienes playlists: usa \`/new-playlist\` para importar una.`);
            }

            const check = checkVoice(interaction, manager, t, { requireQueue: false, join: true });
            if (check.error) return say(interaction, check.error);

            try {
                const outcome = await manager.play({
                    voiceChannel: check.channel,
                    textChannel: panelChannel(interaction),
                    member: interaction.member,
                    query: match.url,
                });
                const { resolved } = outcome;
                if (resolved instanceof Playlist) {
                    return say(interaction, `🎵 Playlist **${match.name}**: ${resolved.songs.length} temas añadidos a la cola.${outcome.truncated ? ' (lista recortada al máximo permitido)' : ''}`);
                }
                return say(interaction, `🎵 Reproduciendo **${resolved.name}** de tu playlist **${match.name}**.`);
            } catch (error) {
                const key = manager.explainError(error, Date.now());
                log.warn(`Failed to play playlist "${match.name}": ${error.message}`);
                return say(interaction, t(key));
            }
        }

        // ─── Listar ───
        const q = filter.toLowerCase();
        const filtered = q
            ? playlists.filter((pl) =>
                String(pl.name).toLowerCase().includes(q) ||
                String(pl.source).toLowerCase().includes(q))
            : playlists;

        const { respond } = require('../handlers/guards');
        return respond(interaction, views.playlistsView(t, {
            playlists: filtered,
            total: playlists.length,
            page,
            filter: filter || null,
        }));
    },
};
