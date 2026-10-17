'use strict';

const { command, queueAction } = require('./_shared');
const actions = require('../music/actions');
const { FILTERS, FILTER_LABELS } = require('../music/filters');

module.exports = {
    category: 'playback',
    data: command('filter', 'Apply an audio filter')
        .addStringOption((option) => option
            .setName('name')
            .setDescription('Filter')
            .setRequired(true)
            .addChoices({ name: 'None', value: 'none' }, ...FILTERS.map((name) => ({ name: FILTER_LABELS[name], value: name })))),

    execute(interaction, ctx) {
        return queueAction(interaction, ctx, (queue, t) => actions.setFilter(queue, t, interaction.options.getString('name', true)));
    },
};
