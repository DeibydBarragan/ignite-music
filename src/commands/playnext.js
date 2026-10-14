'use strict';

const i18n = require('../core/i18n');
const { command } = require('./_shared');
const { playFromInteraction } = require('./play');
const { choices } = require('../music/suggest');
const { say } = require('../handlers/guards');

module.exports = {
    category: 'playback',
    data: command('playnext', 'Play a song right after the current one (jump the queue)')
        .setDescriptionLocalizations(i18n.localizations('commands.playnext.description'))
        .addStringOption((option) => option
            .setName('query')
            .setDescription('Song name or link')
            .setDescriptionLocalizations(i18n.localizations('commands.playnext.query_description'))
            .setRequired(true)
            .setMaxLength(1000)
            .setAutocomplete(true)),

    async execute(interaction, ctx) {
        await interaction.deferReply({ flags: 'Ephemeral' });
        const raw = interaction.options.getString('query') ||
                    interaction.options.getString('song') ||
                    interaction.options.getString('name') ||
                    interaction.options.data[0]?.value;
        const query = typeof raw === 'string' ? raw.trim() : '';
        if (!query) {
            return say(interaction, ctx.t('commands.playnext.query_description') || 'Debes ingresar una canción o enlace.');
        }
        return playFromInteraction(interaction, ctx, query, { next: true });
    },

    async autocomplete(interaction) {
        await interaction.respond(await choices(interaction.options.getFocused()));
    },
};
