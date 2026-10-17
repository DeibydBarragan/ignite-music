-- ==============================================================================
-- 20261016_top_songs.sql — Agregación de top canciones en la base de datos
-- ==============================================================================
-- getTopSongs traía TODAS las filas de song_plays de un guild para agregarlas
-- en memoria (Node.js). Esta función agrega por URL en Postgres y devuelve
-- como máximo 50 filas, evitando transferir miles de registros por petición.
--
-- Las fechas excluidas (data/history/exclusions.json, cacheado en memoria por
-- el bot) se pasan como parámetro para conservar exactamente la semántica
-- anterior (un play en una fecha excluida no suma al ranking).
-- ==============================================================================

CREATE OR REPLACE FUNCTION public.top_songs(
    p_guild_id TEXT,
    p_since TIMESTAMPTZ DEFAULT NULL,
    p_limit INT DEFAULT 50,
    p_excluded_dates TEXT[] DEFAULT '{}'
)
RETURNS TABLE (
    song_id TEXT,
    title TEXT,
    artist TEXT,
    album_art TEXT,
    url TEXT,
    duration INTEGER,
    source TEXT,
    play_count BIGINT,
    last_played TIMESTAMPTZ
)
LANGUAGE SQL
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
    SELECT
        (array_agg(song_id ORDER BY played_at DESC NULLS LAST))[1]  AS song_id,
        (array_agg(title ORDER BY played_at DESC NULLS LAST))[1]    AS title,
        (array_agg(artist ORDER BY played_at DESC NULLS LAST))[1]   AS artist,
        (array_agg(album_art ORDER BY played_at DESC NULLS LAST))[1] AS album_art,
        coalesce(url, song_id, title)                               AS url,
        (array_agg(duration ORDER BY played_at DESC NULLS LAST))[1] AS duration,
        (array_agg(source ORDER BY played_at DESC NULLS LAST))[1]   AS source,
        count(*)                                                    AS play_count,
        max(played_at)                                              AS last_played
    FROM song_plays
    WHERE guild_id = p_guild_id
      AND (p_since IS NULL OR played_at >= p_since)
      AND NOT (played_at::date = ANY (coalesce(p_excluded_dates, '{}')::date[]))
    GROUP BY coalesce(url, song_id, title)
    ORDER BY play_count DESC, last_played DESC
    LIMIT least(greatest(coalesce(p_limit, 50), 1), 50);
$$;

-- Solo el bot (service role) puede ejecutarla.
REVOKE ALL ON FUNCTION public.top_songs(TEXT, TIMESTAMPTZ, INT, TEXT[]) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.top_songs(TEXT, TIMESTAMPTZ, INT, TEXT[]) TO service_role;
