<div align="center">

# Ignite Music

A fast Discord music bot for YouTube, Spotify, SoundCloud and direct links — with a button player panel, sharding, queue resume, saved user playlists (Supabase), and a REST API for the Ignite web dashboard.

You only need **Node.js** and a **bot token**: everything else is installed and checked automatically.

</div>

---

## Contents

1. [Features](#features)
2. [Requirements](#requirements)
3. [Setup (step by step)](#setup-step-by-step)
4. [Starting the bot](#starting-the-bot)
5. [Settings (.env)](#settings-env)
6. [REST API (Ignite dashboard)](#rest-api-ignite-dashboard)
7. [User playlists (Supabase)](#user-playlists-supabase)
8. [Discord commands](#discord-commands)
9. [YouTube: "Sign in to confirm you're not a bot"](#youtube-sign-in-to-confirm-youre-not-a-bot)
10. [What the startup check does](#what-the-startup-check-does)
11. [Updating](#updating)
12. [Troubleshooting](#troubleshooting)
13. [For developers](#for-developers)
14. [Privacy and legal](#privacy-and-legal)
15. [License and credits](#license-and-credits)

---

## Features

| | |
| --- | --- |
| **Sources** | YouTube (videos, Shorts, Music, playlists), Spotify (tracks, albums, playlists, artists), SoundCloud, direct audio links, and everything else yt-dlp supports |
| **Fast start** | Songs are streamed, never saved to disk. Lookup, voice join, and next-song preparation run in parallel |
| **Reliable audio** | yt-dlp resolves audio the way YouTube expects and pipes it to ffmpeg through a local relay, avoiding 403s on direct links. Optional SoundCloud fallback when YouTube refuses a song |
| **Player panel** | Cover art, progress bar, queue preview, control buttons, and an audio filter menu (Discord Components V2) |
| **Saved playlists** | `/new-playlist` imports a Spotify/YouTube/SoundCloud link and stores it per user in Supabase; `/playlists` lists and plays them back |
| **REST API** | Health, guilds, voice status, player state, queue management, playback controls, and playlist resolve — built for the Ignite web dashboard |
| **Sharding** | Always on (auto shard count), crashed shards restart automatically |
| **Resume after restart** | Queue and position are saved and restored |
| **Proxy, cookies, PO token** | Options for hosts that YouTube blocks |
| **23 languages** | Per server, or follows the server's Discord language automatically |
| **No privileged intents** | Only `Guilds` and `GuildVoiceStates` |

---

## Requirements

- **Node.js 22.12+** (LTS from <https://nodejs.org>)
- A Discord application token (see setup below)
- Optional: Spotify app keys (only for Spotify playlists with 100+ songs), Supabase project (only for saved playlists)

You do **not** need to install Python, ffmpeg, or yt-dlp: the bot downloads and updates them by itself.

---

## Setup (step by step)

### 1. Install Node.js

Download and install the **LTS** version from **<https://nodejs.org>** (22.12 or newer). Then close and reopen your terminal.

### 2. Download the bot

```bash
git clone https://github.com/DeibydBarragan/ignite-music.git
cd ignite-music
```

Without git: on GitHub press **Code > Download ZIP** and extract it.

### 3. Create the Discord bot and get its token

1. Open **<https://discord.com/developers/applications>** and press **New Application**.
2. Open the **Bot** page and press **Reset Token**. Copy the token — it is a password, never share it.
3. Open the **OAuth2** page, under **OAuth2 URL Generator** tick `bot` and `applications.commands`, then tick these bot permissions: *View Channels, Send Messages, Embed Links, Read Message History, Connect, Speak*. Open the generated link and add the bot to your server.

No "Privileged Gateway Intents" are needed.

### 4. Put the token into `.env`

Copy `.env.example` to `.env` (the bot also creates it on first start) and fill in:

```env
DISCORD_TOKEN=paste_your_token_here
```

That is the only required setting. Everything else in [Settings](#settings-env) is optional.

> On a hosting panel (Pterodactyl, Railway, VPS with systemd, …) you can set `DISCORD_TOKEN` as an environment variable instead of using a `.env` file.

---

## Starting the bot

All of these do the same thing:

| | |
| --- | --- |
| Windows | double-click **`start.bat`** (or `start-shard.bat`) |
| Any system | `npm start` |
| Any system | `node .` or `node index.js` |

The first start takes longer: it installs npm packages and downloads yt-dlp and ffmpeg. When you see `1 shard(s) running`, the bot is online — use `/play` in your server.

To stop the bot press **Ctrl + C**. The queue is saved and continues when the bot starts again.

```bash
npm test         # run tests
npm run check    # syntax + language check
npm run deploy   # re-register slash commands
```

---

## Settings (.env)

Only `DISCORD_TOKEN` is required. See `.env.example` for the full commented list.

| Setting | Default | What it does |
| --- | --- | --- |
| `DISCORD_TOKEN` | | Bot token (**required**) |
| `CLIENT_ID` | detected from token | Application ID |
| `GUILD_ID` | | Register commands only in this server (instant updates while developing) |
| `STATUS` | `/play` | "Listening to …" text |
| `EMBED_COLOR` | `#5865F2` | Panel color |
| `SUPPORT_SERVER` / `WEBSITE` | | Links shown in `/help` |
| `DEFAULT_LANGUAGE` | `en` | Fallback language (servers also follow their Discord locale automatically) |
| `DEFAULT_VOLUME` | `100` | Starting volume (1–100) |
| `MAX_QUEUE_SIZE` | `500` | Max songs in queue |
| `MAX_PLAYLIST_SIZE` | `200` | Max songs loaded from one playlist |
| `LEAVE_ON_EMPTY` | `60` | Seconds before leaving when nobody is listening (`0` = never) |
| `LEAVE_ON_FINISH` | `120` | Seconds to stay after the queue ends (`0` = never leave) |
| `PANEL_REFRESH` | `20` | Seconds between progress-bar updates (`0` = only on changes) |
| `SPOTIFY_CLIENT_ID` / `SPOTIFY_CLIENT_SECRET` | | Only needed for Spotify playlists with 100+ songs ([create an app](https://developer.spotify.com/dashboard)) |
| `COOKIES_FILE` | | Path to a YouTube `cookies.txt` file (see below) |
| `COOKIES_FROM_BROWSER` | | Read YouTube cookies from a local browser (`chrome`, `firefox`, `edge`, `safari`, …) |
| `YOUTUBE_PO_TOKEN` | | YouTube PO token (advanced) |
| `YOUTUBE_FALLBACK` | `soundcloud` | When YouTube fails for a song, play it from SoundCloud. `off` shows the error instead |
| `YTDLP_EXTRACTOR_ARGS` | | Extra yt-dlp `--extractor-args` (advanced) |
| `YTDLP_CONCURRENCY` | `4` | Simultaneous yt-dlp processes per shard |
| `YTDLP_PATH` / `FFMPEG_PATH` | auto-downloaded | Use your own binaries |
| `PROXY_URL` | | Proxy for YouTube/audio (comma-separated list; each guild sticks to one) |
| `TOTAL_SHARDS` | `auto` | Shard count |
| `SHARD_MODE` | `process` | `process` or `worker` |
| `DEBUG` | `false` | Verbose logs |
| `SUPABASE_URL` / `SUPABASE_KEY` | | Enables saved user playlists (`/new-playlist`, `/playlists`) |
| `API_ENABLED` | `true` | Internal REST API for the Ignite dashboard |
| `API_PORT` | `3001` | REST API port |
| `API_SECRET` | | Shared secret for the REST API (`Authorization: Bearer <secret>`) |

### Proxy

```env
PROXY_URL=http://user:password@host:port
PROXY_URL=http://proxy-a:8080,http://proxy-b:8080
```

HTTP(S) and SOCKS proxies both work. YouTube locks a stream to the requesting IP, so each guild always uses the same proxy from the list.

---

## REST API (Ignite dashboard)

The bot exposes a small REST API (Express) used by the Ignite web frontend. It is enabled by default (`API_ENABLED=true`, `API_PORT=3001`).

Authentication: every `/api/*` endpoint **except** `GET /api/health` requires the shared secret:

```http
Authorization: Bearer <API_SECRET>
```

(or `X-API-Key: <API_SECRET>`).

| Method | Endpoint | Description |
| --- | --- | --- |
| `GET` | `/api/health` | Public. Status, uptime, bot tag, guild count |
| `GET` | `/api/guilds` | Servers the bot is in |
| `GET` | `/api/guilds/:guildId/voice-status?userId=` | Bot/user voice channels + whether the user can control playback |
| `GET` | `/api/guilds/:guildId/player` | Player state + current song + upcoming queue (Ignite frontend song contract) |
| `GET` | `/api/guilds/:guildId/playlist/resolve?url=` | Resolve a playlist/track URL without joining voice (import preview: `kind`, `name`, `source`, `tracks`, `truncated`) |
| `POST` | `/api/guilds/:guildId/play` | Play or enqueue. Body: `{ query, voiceChannelId?, userId?, next?, skip? }`. Returns `song` and, for playlists, `{ name, count, truncated }` |
| `POST` | `/api/guilds/:guildId/control` | Playback actions. Body: `{ action, value? }` where action is `pause`, `resume`, `toggle`, `skip`, `previous`, `stop`, `shuffle`, `autoplay`, `volume` (value: 0–100), `seek` (value: seconds), `loop` (value: `off`/`track`/`queue`) |
| `DELETE` | `/api/guilds/:guildId/queue/:index` | Remove one upcoming song (1-based index into the queue) |
| `DELETE` | `/api/guilds/:guildId/queue` | Clear upcoming songs (keeps the current one) |

Example:

```bash
curl -H "Authorization: Bearer $API_SECRET" \
  http://localhost:3001/api/guilds/123456789012345678/player
```

---

## User playlists (Supabase)

`/new-playlist` and `/playlists` persist each user's imported playlists in Supabase so they survive restarts and work from both Discord and the web dashboard.

1. Create a free project at <https://supabase.com> and run this SQL once:

```sql
create table if not exists user_playlists (
  id uuid default gen_random_uuid() primary key,
  user_id text not null,
  name text not null,
  source text not null default 'spotify',
  url text not null,
  cover text,
  tracks jsonb not null default '[]'::jsonb,
  created_at timestamptz not null default now(),
  unique (user_id, url)
);
```

2. Add the credentials to `.env`:

```env
SUPABASE_URL=https://xyzcompany.supabase.co
SUPABASE_KEY=your_anon_or_service_key
```

3. Restart the bot. Import with `/new-playlist url:<link> [nombre:<name>]`, list with `/playlists`, play one back with `/playlists reproducir:<name>`.

Without these keys the music playback and REST API still work — only the saved-playlist commands report that the database is unavailable.

---

## Discord commands

| Command | What it does |
| --- | --- |
| `/play <song or link> [next]` | Play a song, playlist, or link. Suggestions appear while typing. `next` queues it right after the current song |
| `/search <text>` | Pick a song from YouTube results |
| `/new-playlist <url> [nombre]` | Import a Spotify/YouTube/SoundCloud link into your saved collection (ephemeral) |
| `/playlists [nombre] [reproducir] [page]` | List your saved playlists; `reproducir:<name or url>` plays one |
| `/nowplaying` | Move the player panel to the bottom of the chat |
| `/pause` `/resume` `/skip` `/previous` `/stop` | Playback |
| `/seek <time>` | Jump to a time, e.g. `1:30` |
| `/volume <0-100>` | Volume |
| `/loop <off/song/queue>` | Repeat mode |
| `/autoplay` | Keep playing related songs when the queue ends |
| `/filter <name>` | Bass Boost, Nightcore, Vaporwave, 8D, Karaoke, Echo, Surround, Tremolo |
| `/queue [page]` | Show the queue |
| `/remove` `/move` `/jump` `/clear` `/shuffle` | Edit the queue |
| `/lyrics [song]` | Lyrics |
| `/language` | Server language (needs Manage Server) |
| `/help` | All commands and bot statistics |

The player panel has buttons for the same actions. Only people in the bot's voice channel can control the music. When everyone leaves, the music pauses and resumes when someone comes back.

---

## YouTube: Sign in to confirm you're not a bot

YouTube blocks many VPS / datacenter IPs. When that happens the bot tells you in Discord and in the console. Fix it with cookies from a YouTube account (a secondary account is recommended):

1. In Chrome or Edge install the extension **"Get cookies.txt LOCALLY"** (Firefox: **"cookies.txt"**).
2. Open **youtube.com**, log in, play any video.
3. Click the extension and export the cookies. Save the file as **`cookies.txt`** in the bot folder.
4. Add this line to `.env` and restart the bot:

```env
COOKIES_FILE=./cookies.txt
```

If the bot runs on the same machine where you use YouTube, `COOKIES_FROM_BROWSER=chrome` (or `firefox`, `edge`, `safari`) works without exporting anything. Alternatives: `YOUTUBE_PO_TOKEN` or `PROXY_URL` with a residential proxy.

yt-dlp updates itself once a day, which fixes most other YouTube errors.

---

## What the startup check does

Every start begins with a checklist. It fixes what it can by itself and stops with clear instructions for anything else, so the bot never runs half-broken:

```text
Ignite Music startup check

  [ OK ] Node.js 22.12.0
  [ OK ] npm packages
  [ OK ] Discord token  Ignite Music (application 123456789012345678)
  [ OK ] yt-dlp  2026.09.01
  [ OK ] ffmpeg  ffmpeg version 7.1
  [ OK ] Voice libraries  opus, encryption
  [INFO] Spotify API keys not set  optional: large Spotify playlists load only their first 100 songs
```

| Check | Fixed automatically | If it cannot be fixed |
| --- | --- | --- |
| Node.js version | | Stops and tells you to install Node.js 22.12+ |
| npm packages | Runs `npm install` when packages are missing or after an update | Stops and shows the reinstall commands |
| `.env` file | Created from `.env.example` | Stops and explains how to get the token |
| Discord token | `CLIENT_ID` detected from the token | Stops when the token is empty or rejected by Discord |
| yt-dlp | Downloaded (no Python needed) and updated daily | Stops and shows how to install it manually |
| ffmpeg | Downloaded | Stops and shows how to install it manually |
| Voice libraries | | Stops and tells you to delete `node_modules` |
| `PROXY_URL`, `COOKIES_FILE`, `GUILD_ID` | | Stops when a value is invalid or the file is missing |

---

## Updating

**With git:**

```bash
git pull
npm start
```

**From a ZIP:** download the new ZIP, extract it, and copy your **`.env`** file (plus the `data/` folder to keep server settings) from the old folder into the new one. Then start the bot.

The startup check reinstalls new packages by itself, so `npm install` is not needed after an update.

---

## Troubleshooting

| Problem | Solution |
| --- | --- |
| `'node' is not recognized` / `command not found: node` | Node.js is not installed or the terminal predates the install. Install it from <https://nodejs.org> and open a new terminal |
| The bot stops with `[FAIL]` lines | Follow what the lines say — each one explains its fix |
| Commands do not show up in Discord | Wait a minute and press **Ctrl + R** in Discord. Check the bot was invited with `applications.commands`. If `GUILD_ID` is set, the bot must be in that server |
| The bot joins but there is no sound | Give the bot **Speak** permission in that channel, make sure it is not server-muted, and check `/volume` |
| "YouTube blocked this request (bot detection)" | See [YouTube section](#youtube-sign-in-to-confirm-youre-not-a-bot) |
| A song is skipped with "Audio stream failed" | The console shows the real reason. `HTTP Error 403` / "Sign in to confirm" means YouTube is blocking this IP — use the cookies fix. Meanwhile songs fall back to SoundCloud (`YOUTUBE_FALLBACK`) |
| Songs play from SoundCloud instead of YouTube | YouTube refused them; the console says why (usually IP blocking). Use the cookies fix |
| Saved playlists say the database is unavailable | Set `SUPABASE_URL` and `SUPABASE_KEY` and create the `user_playlists` table (see above) |
| REST API returns 401 | Send `Authorization: Bearer <API_SECRET>` with the same secret as the bot's `.env` |
| "Voice libraries do not load" | `node_modules` was copied from another machine. Delete `node_modules` and start again |
| Anything else | Set `DEBUG=true` in `.env` and read the log |

---

## For developers

```text
index.js               Entry point: startup check, then the sharding manager
shard.js               Shard entry (spawned by the manager)
src/
  setup/               Startup check (bootstrap needs no npm packages), launcher, preflight
  bot.js               One shard: Discord client, events, graceful shutdown (+ REST API attach)
  config.js            Reads .env
  deploy.js            Slash command registration (only when commands changed)
  commands/            Slash commands (playback, playlists, queue, info)
  handlers/           Interaction router and permission/voice guards
  music/               DisTube setup, YouTube extractor, yt-dlp runner, audio relay,
                       sessions, lyrics, saved playlists (playlists.js)
  api/server.js        Express REST API for the Ignite dashboard
  ui/views.js          Components V2 layouts (player, queue, search, playlists, help)
  core/                Logger, i18n, cache, JSON store, formatting, yt-dlp/ffmpeg download,
                       DNS fix, Supabase client
languages/             23 translations (per-server)
test/                  Tests (node --test)
bin/                   yt-dlp binaries (downloaded automatically, git-ignored)
data/                  Server settings and saved queues (created automatically, git-ignored)
database/              Local JSON state (languages, player state)
```

---

## Privacy and legal

- [Privacy Policy](PRIVACY_POLICY.md)
- [Terms of Service](TERMS_OF_SERVICE.md)

## License and credits

MIT — see [LICENSE](LICENSE).

Built on [discord.js](https://discord.gg/discordjs) v14 + [DisTube](https://distube.js.org) v5. Forked from [umutxyp/musicbot](https://github.com/umutxyp/musicbot) and extended for the Ignite project (REST API, Supabase playlists, Components V2 panels).
