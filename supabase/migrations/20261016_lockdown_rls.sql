-- ==============================================================================
-- 20261016_lockdown_rls.sql — Cierre del RLS permisivo (Fase 0, seguridad P0)
-- ==============================================================================
-- Objetivo:
--   1. song_plays deja de ser de lectura/escritura pública (USING (true)).
--   2. bot_status queda definida con RLS: lectura pública para el descubrimiento
--      de la API y escritura reservada al service role del bot.
--   3. user_playlists queda definida con RLS estricto y unicidad (user_id, url).
--
-- IMPORTANTE: tras aplicar esta migración el bot DEBE arrancar con
-- SUPABASE_SERVICE_ROLE_KEY configurada (bypassa RLS). El dashboard Next.js
-- conserva acceso a las playlists del propio usuario autenticado mediante las
-- políticas de propietario (sección 3b); bot_status sigue siendo de lectura
-- pública para el descubrimiento de la API.
-- ==============================================================================

-- 0) Eliminar TODAS las políticas existentes en las tablas afectadas,
--    incluidas las creadas a mano con nombres desconocidos.
DO $$
DECLARE r RECORD;
BEGIN
    FOR r IN
        SELECT policyname, tablename
        FROM pg_policies
        WHERE schemaname = 'public'
          AND tablename IN ('song_plays', 'bot_status', 'user_playlists')
    LOOP
        EXECUTE format('DROP POLICY IF EXISTS %I ON public.%I', r.policyname, r.tablename);
    END LOOP;
END $$;

-- 1) song_plays: sin políticas nuevas. SELECT/INSERT/UPDATE/DELETE quedan
--    denegados para anon y authenticated; solo el service role (que ignora
--    RLS) puede operar.

-- 2) bot_status: esquema formal + descubrimiento de la API
CREATE TABLE IF NOT EXISTS bot_status (
    id TEXT PRIMARY KEY,
    api_url TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Compatibilidad con la tabla creada a mano en versiones anteriores (sin updated_at)
ALTER TABLE bot_status ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE bot_status ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Allow public select on bot_status"
    ON bot_status FOR SELECT
    USING (true);

-- Escritura: solo service role. No se crean políticas INSERT/UPDATE/DELETE para
-- anon/authenticated, de modo que nadie puede secuestrar bot_status.api_url.

-- 3) user_playlists: esquema formal + RLS estricto
CREATE TABLE IF NOT EXISTS user_playlists (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'generic',
    url TEXT NOT NULL,
    cover TEXT,
    tracks JSONB NOT NULL DEFAULT '[]'::jsonb,
    synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Compatibilidad con la tabla creada a mano en versiones anteriores
-- (sin defaults en id/source/tracks).
ALTER TABLE user_playlists ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE user_playlists ALTER COLUMN source SET DEFAULT 'generic';
ALTER TABLE user_playlists ALTER COLUMN tracks SET DEFAULT '[]'::jsonb;
ALTER TABLE user_playlists ADD COLUMN IF NOT EXISTS cover TEXT;
ALTER TABLE user_playlists ADD COLUMN IF NOT EXISTS synced_at TIMESTAMPTZ;
ALTER TABLE user_playlists ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE UNIQUE INDEX IF NOT EXISTS idx_user_playlists_user_url
    ON user_playlists (user_id, url);
CREATE INDEX IF NOT EXISTS idx_user_playlists_user_created
    ON user_playlists (user_id, created_at DESC);

ALTER TABLE user_playlists ENABLE ROW LEVEL SECURITY;
-- Sin políticas para anon: lectura/escritura vía service role (bot) o
-- self-service del propietario autenticado (sección 3b).

-- 3b) Acceso self-service del propietario autenticado (dashboard Next.js).
-- La identidad se resuelve contra auth.identities (provider_id de Discord),
-- que el usuario no puede falsificar desde el cliente, a diferencia de
-- user_metadata (editable vía supabase.auth.updateUser).
CREATE OR REPLACE FUNCTION public.owns_playlist_row(p_user_id TEXT)
RETURNS BOOLEAN
LANGUAGE SQL
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
    SELECT EXISTS (
        SELECT 1 FROM auth.identities i
        WHERE i.user_id = (SELECT auth.uid())
          AND i.provider_id = p_user_id
    );
$$;

REVOKE ALL ON FUNCTION public.owns_playlist_row(TEXT) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.owns_playlist_row(TEXT) TO authenticated;

CREATE POLICY "user_playlists: owner select"
    ON user_playlists FOR SELECT TO authenticated
    USING (public.owns_playlist_row(user_id));

CREATE POLICY "user_playlists: owner insert"
    ON user_playlists FOR INSERT TO authenticated
    WITH CHECK (public.owns_playlist_row(user_id));

CREATE POLICY "user_playlists: owner update"
    ON user_playlists FOR UPDATE TO authenticated
    USING (public.owns_playlist_row(user_id))
    WITH CHECK (public.owns_playlist_row(user_id));

CREATE POLICY "user_playlists: owner delete"
    ON user_playlists FOR DELETE TO authenticated
    USING (public.owns_playlist_row(user_id));
