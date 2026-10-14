-- ==============================================================================
-- Tabla: song_plays (Historial y Canciones Más Sonadas para Ignite Music)
-- ==============================================================================

CREATE TABLE IF NOT EXISTS song_plays (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    guild_id TEXT NOT NULL,
    song_id TEXT,
    title TEXT NOT NULL,
    artist TEXT,
    album_art TEXT,
    url TEXT NOT NULL,
    duration INTEGER DEFAULT 0,
    source TEXT DEFAULT 'youtube',
    user_id TEXT,
    username TEXT,
    played_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices de alto rendimiento para filtrado por servidor y período de tiempo
CREATE INDEX IF NOT EXISTS idx_song_plays_guild_played ON song_plays(guild_id, played_at DESC);
CREATE INDEX IF NOT EXISTS idx_song_plays_url ON song_plays(url);
CREATE INDEX IF NOT EXISTS idx_song_plays_guild_url ON song_plays(guild_id, url);

-- Políticas de Seguridad a Nivel de Fila (RLS)
ALTER TABLE song_plays ENABLE ROW LEVEL SECURITY;

-- Permitir lectura y escritura pública anónima desde el bot y el dashboard
CREATE POLICY "Allow public select on song_plays" 
    ON song_plays FOR SELECT 
    USING (true);

CREATE POLICY "Allow public insert on song_plays" 
    ON song_plays FOR INSERT 
    WITH CHECK (true);
