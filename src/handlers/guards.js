'use strict';

const { PermissionFlagsBits, MessageFlags, RESTJSONErrorCodes } = require('discord.js');
const { isTextChannelInstance } = require('distube');
const views = require('../ui/views');
const { isDevUser } = require('../core/dev-mode');

function findAvailableVoiceChannel(guild) {
    if (!guild?.channels?.cache) return null;
    const me = guild.members.me;
    const voiceChannels = guild.channels.cache.filter((c) => {
        if (!c.isVoiceBased()) return false;
        if (!me) return true;
        const perms = c.permissionsFor(me);
        return perms?.has([PermissionFlagsBits.ViewChannel, PermissionFlagsBits.Connect, PermissionFlagsBits.Speak]);
    });

    return voiceChannels.filter((c) => c.members.some((m) => !m.user.bot)).first()
        || voiceChannels.first()
        || null;
}

/**
 * Check that the user may control music: in a voice channel, and in the bot's channel if it is connected.
 * `join: true` also verifies the bot can connect and speak there.
 */
function checkVoice(interaction, manager, t, { requireQueue = true, join = false } = {}) {
    const guild = interaction.guild;
    const userId = interaction.user?.id;
    const isDev = isDevUser(userId);
    let channel = interaction.member?.voice?.channel;
    if (!channel) {
        if (isDev) {
            const botVoiceChannel = guild.members.me?.voice?.channel;
            const existingQueue = manager.getQueue(guild.id);
            channel = botVoiceChannel
                || existingQueue?.voiceChannel
                || (join ? findAvailableVoiceChannel(guild) : null);

            if (!channel) return { error: t('commands.play.voice_channel_required') };
        } else {
            return { error: t('commands.play.voice_channel_required') };
        }
    }

    const botChannelId = guild.members.me?.voice?.channelId;
    if (botChannelId && botChannelId !== channel.id && !isDev) return { error: t('commands.play.same_channel_required') };

    const queue = manager.getQueue(guild.id);
    if (requireQueue && (!queue || !queue.songs.length)) return { error: t('commands.nowplaying.no_track') };

    if (join && !botChannelId) {
        const permissions = channel.permissionsFor(guild.members.me);
        if (!permissions?.has([PermissionFlagsBits.ViewChannel, PermissionFlagsBits.Connect, PermissionFlagsBits.Speak])) {
            return { error: t('commands.play.no_permissions') };
        }
        if (channel.full && !permissions.has(PermissionFlagsBits.MoveMembers)) return { error: t('ui.voice_full') };
    }
    return { channel, queue };
}

/**
 * Text channel DisTube may post the panel in (or undefined when the bot can't write there).
 */
function panelChannel(interaction) {
    const channel = interaction.channel;
    // Exactly the channels DisTube accepts as a queue text channel.
    if (!channel?.isTextBased() || !isTextChannelInstance(channel)) return undefined;
    const permissions = channel.permissionsFor?.(interaction.guild.members.me);
    if (!permissions?.has([PermissionFlagsBits.ViewChannel, PermissionFlagsBits.SendMessages])) return undefined;
    if (channel.isThread() && !permissions.has(PermissionFlagsBits.SendMessagesInThreads)) return undefined;
    return channel;
}

const IGNORED_CODES = new Set([RESTJSONErrorCodes.UnknownInteraction, RESTJSONErrorCodes.InteractionHasAlreadyBeenAcknowledged]);

/**
 * Reply, follow up or edit depending on the interaction state. Never throws for expired interactions.
 */
async function respond(interaction, payload) {
    try {
        if (interaction.deferred && !interaction.replied) {
            const { flags, ...rest } = payload;
            return await interaction.editReply({ ...rest, flags: flags & MessageFlags.IsComponentsV2 ? MessageFlags.IsComponentsV2 : undefined });
        }
        if (interaction.replied) return await interaction.followUp(payload);
        return await interaction.reply(payload);
    } catch (error) {
        if (IGNORED_CODES.has(error.code)) return null;
        throw error;
    }
}

const say = (interaction, message, options) => respond(interaction, views.notice(message, options));

module.exports = { checkVoice, panelChannel, respond, say, IGNORED_CODES };
