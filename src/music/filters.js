'use strict';

/**
 * Audio filter catalog (Phase 2.3): moved out of ui/views.js so domain code
 * (actions.setFilter, /filter command) no longer depends on the UI layer.
 * ui/views.js re-exports both for backward compatibility.
 */

const FILTERS = ['bassboost', 'nightcore', 'vaporwave', '3d', 'karaoke', 'echo', 'surround', 'tremolo'];

const FILTER_LABELS = {
    bassboost: 'Bass Boost',
    nightcore: 'Nightcore',
    vaporwave: 'Vaporwave',
    '3d': '8D',
    karaoke: 'Karaoke',
    echo: 'Echo',
    surround: 'Surround',
    tremolo: 'Tremolo',
};

module.exports = { FILTERS, FILTER_LABELS };
