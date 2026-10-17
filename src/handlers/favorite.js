'use strict';

const views = require('../ui/views');
const { supabase } = require('../core/supabase');
const log = require('../core/logger').createLogger('favorite');

async function handleFavorite(interaction, song) {
    if (!song) {
        return interaction.reply(views.notice('No hay ninguna canción reproduciéndose.', { ephemeral: true }));
    }
    if (!supabase) {
        return interaction.reply(views.notice('Base de datos no disponible temporalmente.', { ephemeral: true }));
    }

    const userId = String(interaction.user.id);
    const songId = String(song.id || song.url);
    const songUrl = String(song.url || '');

    try {
        const [bySong, byUrl] = await Promise.all([
            supabase
                .from('user_favorites')
                .select('id')
                .eq('user_id', userId)
                .eq('song_id', songId)
                .limit(1)
                .maybeSingle(),
            songUrl
                ? supabase
                    .from('user_favorites')
                    .select('id')
                    .eq('user_id', userId)
                    .eq('url', songUrl)
                    .limit(1)
                    .maybeSingle()
                : Promise.resolve({ data: null, error: null }),
        ]);

        const existing = bySong.data || byUrl.data;

        if (existing) {
            await supabase
                .from('user_favorites')
                .delete()
                .eq('id', existing.id);

            return interaction.reply(views.notice(
                `💔 **${song.name}** se ha eliminado de tus favoritos.`,
                { ephemeral: true }
            ));
        } else {
            await supabase
                .from('user_favorites')
                .insert({
                    user_id: userId,
                    song_id: songId,
                    title: song.name,
                    artist: song.uploader?.name || 'Desconocido',
                    album_art: song.thumbnail || null,
                    url: songUrl,
                    duration: song.duration || 0,
                    source: song.source || 'youtube',
                });

            return interaction.reply(views.notice(
                `⭐ ¡**${song.name}** guardada en tus favoritos!\nYa está en tu catálogo web para escucharla cuando quieras.`,
                { ephemeral: true }
            ));
        }
    } catch (err) {
        log.error('Error toggling favorite in Discord interaction:', err.message);
        return interaction.reply(views.notice('Ocurrió un error al actualizar tus favoritos.', { ephemeral: true }));
    }
}

module.exports = { handleFavorite };
