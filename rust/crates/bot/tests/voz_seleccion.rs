use bot::api::{contexto::ProveedorApi, proveedor_real::ProveedorApiReal};
use medios::ytdlp_args::YtDlpSettings;
use serde_json::{json, Value};
use std::sync::Arc;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::gateway::payload::incoming::{ChannelCreate, GuildCreate, VoiceStateUpdate};

const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";
const FIRST: &str = "111111111111111111";
const SECOND: &str = "222222222222222222";
const BOT: &str = "333333333333333333";

fn estado_voz(cache: &InMemoryCache, uid: &str, canal: &str, bot: bool) {
    let evento: VoiceStateUpdate = serde_json::from_value(json!({
        "guild_id": GUILD, "channel_id": canal, "user_id": uid,
        "session_id": "offline", "deaf": false, "mute": false,
        "self_deaf": bot, "self_mute": false, "self_video": false, "suppress": false,
        "member": {
            "user": {"id":uid,"username":"listener","discriminator":"0","avatar":null,"bot":bot},
            "roles":[],"deaf":false,"mute":false,"flags":0,
            "joined_at":"2024-01-01T00:00:00.000000+00:00"
        }
    }))
    .unwrap();
    cache.update(&evento);
}

#[tokio::test]
async fn canales_y_guardas_play_coinciden_con_node() {
    let cache = Arc::new(bot::cliente::crear_cache());
    cache.update(&GuildCreate::Available(guild()));
    for (id, kind, name, position, bitrate, limit) in [
        (FIRST, 2, "General", 8, 64000, 26),
        (SECOND, 13, "Stage", 1, 96000, 0),
        ("555555555555555555", 0, "chat", 0, 0, 0),
    ] {
        let evento: ChannelCreate = serde_json::from_value(json!({
            "id":id,"guild_id":GUILD,"type":kind,"name":name,"position":position,
            "bitrate":bitrate,"user_limit":limit
        }))
        .unwrap();
        cache.update(&evento);
    }
    estado_voz(&cache, BOT, FIRST, true);
    estado_voz(&cache, "444444444444444444", FIRST, true);
    let proveedor = ProveedorApiReal::nuevo(
        cache.clone(),
        None,
        None,
        Arc::new(player::GestorColas::nuevo()),
        None,
        BOT.parse().unwrap(),
        YtDlpSettings::default(),
        Vec::new(),
        None,
        100,
        100,
    );
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/discovery-play.json"
    ))
    .unwrap();
    let play = || proveedor.reproducir(GUILD, "test query", None, Some(USER), false, false, false);
    let (status, code, detail) = play().await.unwrap_err();
    assert_eq!(
        json!({"status":status,"code":code,"detail":detail}),
        fixture["playErrors"]["outside"]
    );
    estado_voz(&cache, USER, SECOND, false);
    let (status, code, detail) = play().await.unwrap_err();
    assert_eq!(
        json!({"status":status,"code":code,"detail":detail}),
        fixture["playErrors"]["different"]
    );
    assert!(!proveedor.gestor_colas.tiene_cola(GUILD).await);

    estado_voz(&cache, USER, FIRST, false);
    let canales = proveedor.obtener_canales(GUILD).await.unwrap();
    assert_eq!(
        json!({"guildId":GUILD,"total":canales.len(),"channels":canales}),
        fixture["channels"]
    );
    let voz = proveedor.estado_voz(GUILD, Some(USER)).await.unwrap();
    let bot = voz.bot.unwrap();
    assert!(bot.deaf && bot.self_deaf && !bot.server_deaf && voz.can_control);
    for i in 0..25 {
        estado_voz(
            &cache,
            &(999999999999999999u64 - i).to_string(),
            FIRST,
            false,
        );
    }
    let canales = proveedor.obtener_canales(GUILD).await.unwrap();
    assert_eq!(canales[1].user_count, 26);
    assert_eq!(canales[1].members.len(), 25);
    assert_eq!(canales[1].members[0].id, USER);
}

fn channel(cache: &InMemoryCache, id: &str, kind: u8) {
    let event: ChannelCreate = serde_json::from_value(json!({
        "id": id, "guild_id": GUILD, "type": kind, "name": id
    }))
    .unwrap();
    cache.update(&event);
}

fn channel_deny_connect(cache: &InMemoryCache, id: &str) {
    let event: ChannelCreate = serde_json::from_value(json!({
        "id": id, "guild_id": GUILD, "type": 2, "name": id,
        "permission_overwrites": [{ "id": GUILD, "type": 0, "allow": "0", "deny": "1048576" }]
    }))
    .unwrap();
    cache.update(&event);
}

fn guild() -> twilight_model::guild::Guild {
    serde_json::from_value(json!({
        "id": GUILD, "name": "Test", "owner_id": USER, "unavailable": false,
        "afk_timeout": 300, "default_message_notifications": 0, "explicit_content_filter": 0,
        "features": [], "mfa_level": 0, "verification_level": 0, "nsfw_level": 0,
        "preferred_locale": "en-US", "premium_tier": 0, "system_channel_flags": 0,
        "premium_progress_bar_enabled": false, "large": false, "emojis": [], "stickers": [],
        "roles": [{"id": GUILD, "name": "@everyone", "permissions": "1024", "color": 0, "colors": {"primary_color": 0},
                   "hoist": false, "managed": false, "mentionable": false, "position": 0, "flags": 0}],
        "members": [{"user": {"id": FIRST, "username": "bot", "discriminator": "0", "avatar": null, "bot": true},
                     "roles": [], "deaf": false, "mute": false, "flags": 0, "joined_at": "2024-01-01T00:00:00.000000+00:00"}],
        "channels": [], "voice_states": [], "presences": [], "threads": [], "stage_instances": [],
        "guild_scheduled_events": []
    })).unwrap()
}

#[tokio::test]
async fn unirse_rechaza_canal_sin_permiso_connect() {
    let cache = Arc::new(InMemoryCache::new());
    let guild = guild();
    cache.update(&GuildCreate::Available(guild.clone()));
    channel(&cache, SECOND, 2);
    let provider = ProveedorApiReal::nuevo(
        cache.clone(),
        None,
        None,
        Arc::new(player::GestorColas::nuevo()),
        None,
        FIRST.parse().unwrap(),
        YtDlpSettings::default(),
        vec![],
        None,
        100,
        100,
    );
    let error = provider
        .unirse_voz(GUILD, Some(SECOND), None)
        .await
        .unwrap_err();
    assert_eq!((error.0, error.1), (403, "voice_permission_denied"));
    let mut permitida = guild;
    permitida.roles[0].permissions |= twilight_model::guild::Permissions::CONNECT;
    cache.update(&GuildCreate::Available(permitida));
    channel_deny_connect(&cache, SECOND);
    let error = provider
        .unirse_voz(GUILD, Some(SECOND), None)
        .await
        .unwrap_err();
    assert_eq!((error.0, error.1), (403, "voice_permission_denied"));

    // A channel overwrite can deny CONNECT even when @everyone grants it.
    channel(&cache, SECOND, 2);
    assert_eq!(
        provider
            .unirse_voz(GUILD, Some(SECOND), None)
            .await
            .unwrap()
            .id,
        SECOND
    );
}

#[tokio::test]
async fn seleccionar_canal_desde_eventos_de_cache() {
    let cache = Arc::new(InMemoryCache::new());
    channel(&cache, FIRST, 2);
    channel(&cache, SECOND, 2);
    let state: VoiceStateUpdate = serde_json::from_value(json!({
        "guild_id": GUILD, "channel_id": SECOND, "user_id": USER,
        "session_id": "session", "deaf": false, "mute": false,
        "self_deaf": false, "self_mute": false, "self_video": false,
        "suppress": false
    }))
    .unwrap();
    cache.update(&state);
    let provider = ProveedorApiReal::nuevo(
        cache,
        None,
        None,
        Arc::new(player::GestorColas::nuevo()),
        None,
        0,
        YtDlpSettings::default(),
        vec![],
        None,
        100,
        100,
    );
    assert_eq!(
        provider
            .unirse_voz(GUILD, Some(FIRST), Some(USER))
            .await
            .unwrap()
            .id,
        FIRST
    );
    assert_eq!(
        provider
            .unirse_voz(GUILD, None, Some(USER))
            .await
            .unwrap()
            .id,
        SECOND
    );
    assert_eq!(
        provider.unirse_voz(GUILD, None, None).await.unwrap().id,
        FIRST
    );
}
