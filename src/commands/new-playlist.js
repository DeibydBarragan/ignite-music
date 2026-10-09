'use strict';

const { command } = require('./_shared');
const { say } = require('../handlers/guards');
const { resolvePlaylist, upsertUserPlaylist, deriveName } = require('../music/playlists');
const log = require('../core/logger').createLogger('new-playlist-cmd');

function isHttpUrl(text) {
    const value = String(text || '').trim().toLowerCase();
    return value.startsWith('http://') || value.startsWith('https://');
}

module.exports = {
    category: 'playback',
    data: command('new-playlist', 'Importar una playlist por enlace y guardarla en tu colección')
        .addStringOption((option) => option
            .setName('url')
            .setDescription('Enlace de Spotify, YouTube o SoundCloud')
            .setRequired(true)
            .setMaxLength(1000))
        .addStringOption((option) => option
            .setName('nombre')
            .setDescription('Nombre personalizado para la playlist')
            .setMaxLength(60)),

    async execute(interaction, { t, manager }) {
        await interaction.deferReply({ flags: 'Ephemeral' });
        const url = (interaction.options.getString('url') || '').trim();
        const customName = (interaction.options.getString('nombre') || '').trim();

        if (!isHttpUrl(url)) {
            return say(interaction, 'Pega un enlace válido de Spotify, YouTube o SoundCloud.');
        }

        // 1. Resolver sin unirse a voz (dry-run)
        let resolved;
        try {
            resolved = await resolvePlaylist(manager, interaction.guild, url, {
                member: interaction.member,
            });
        } catch (error) {
            const key = manager.explainError(error, Date.now());
            log.warn(`Failed to resolve playlist "${url}": ${error.message}`);
            return say(interaction, t(key));
        }

        if (!resolved.tracks.length) {
            return say(interaction, 'No encontré canciones en ese enlace. Revisa que la playlist sea pública.');
        }

        // 2. Guardar en Supabase (upsert por usuario + url, con snapshot inicial)
        const name = deriveName(url, customName || resolved.name);
        try {
            const { playlist, created } = await upsertUserPlaylist({
                userId: interaction.user.id,
                name,
                url,
                source: resolved.source,
                cover: resolved.tracks[0]?.albumArt,
                tracks: resolved.tracks,
                synced_at: new Date().toISOString(),
            });
            const verb = created ? 'guardada' : 'actualizada';
            const extra = resolved.truncated ? ' (recortada al máximo permitido)' : '';
            return say(interaction,
                `✅ Playlist **${playlist.name}** ${verb}: ${playlist.tracks.length} temas${extra}.\n` +
                `Vela con \`/playlists\` o suénala con \`/playlists reproducir:${playlist.name}\`.`);
        } catch (error) {
            log.error(`Failed to save playlist for ${interaction.user.id}: ${error.message}`);
            return say(interaction, 'Resolví la playlist pero no pude guardarla en la base de datos. Inténtalo de nuevo.');
        }
    },
};
