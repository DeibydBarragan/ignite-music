-- ==============================================================================
-- 20261019_voice_commands.sql — Schema for customizable voice commands & TTS feedback
-- ==============================================================================

-- 1. Table: guild_voice_settings
CREATE TABLE IF NOT EXISTS guild_voice_settings (
    guild_id TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT true,
    wake_word TEXT NOT NULL DEFAULT 'ignite',
    language TEXT NOT NULL DEFAULT 'es',
    tts_feedback_enabled BOOLEAN NOT NULL DEFAULT false,
    tts_templates JSONB NOT NULL DEFAULT '{
        "play": "Reproduciendo {query}",
        "enqueue": "Añadida {query} a la lista",
        "skip": "Saltando canción",
        "pause": "Música pausada",
        "resume": "Reanudando música",
        "stop": "Reproducción detenida",
        "previous": "Canción anterior",
        "shuffle": "Lista en modo aleatorio",
        "volume": "Volumen ajustado al {query}",
        "queue": "Mostrando la lista"
    }'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Table: guild_voice_bindings (custom phrase to command mappings)
CREATE TABLE IF NOT EXISTS guild_voice_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    guild_id TEXT NOT NULL REFERENCES guild_voice_settings(guild_id) ON DELETE CASCADE,
    phrase TEXT NOT NULL,
    command_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(guild_id, phrase)
);

-- 3. Row Level Security (RLS)
ALTER TABLE guild_voice_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE guild_voice_bindings ENABLE ROW LEVEL SECURITY;

-- Allow reading settings for authenticated users and anon dashboard discovery
DROP POLICY IF EXISTS "Allow select on guild_voice_settings" ON guild_voice_settings;
CREATE POLICY "Allow select on guild_voice_settings"
    ON guild_voice_settings FOR SELECT
    USING (true);

DROP POLICY IF EXISTS "Allow select on guild_voice_bindings" ON guild_voice_bindings;
CREATE POLICY "Allow select on guild_voice_bindings"
    ON guild_voice_bindings FOR SELECT
    USING (true);

-- Allow authenticated users and service_role to insert/update settings
DROP POLICY IF EXISTS "Allow authenticated update on guild_voice_settings" ON guild_voice_settings;
DROP POLICY IF EXISTS "Allow service and authenticated modify on guild_voice_settings" ON guild_voice_settings;
CREATE POLICY "Allow service and authenticated modify on guild_voice_settings"
    ON guild_voice_settings FOR ALL
    TO authenticated, service_role
    USING (true)
    WITH CHECK (true);

DROP POLICY IF EXISTS "Allow authenticated modify on guild_voice_bindings" ON guild_voice_bindings;
DROP POLICY IF EXISTS "Allow service and authenticated modify on guild_voice_bindings" ON guild_voice_bindings;
CREATE POLICY "Allow service and authenticated modify on guild_voice_bindings"
    ON guild_voice_bindings FOR ALL
    TO authenticated, service_role
    USING (true)
    WITH CHECK (true);
