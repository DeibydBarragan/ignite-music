# ignite-music — Agent & Contributor Guide

Discord music bot for **Ignite**: YouTube, Spotify and direct audio streaming (discord.js v14 + DisTube v5), always-on sharding, Discord Components V2 controls, Supabase persistence, and a hardened Express 5 REST API + SSE stream for the Next.js dashboard.

---

## 1. Fast Verification & Gates (Mandatory)

Always verify all gates before marking any task as complete:

```powershell
npm run check       # Syntax check across all 129 JS files & 23 language JSONs
npm run lint        # ESLint strict validation (0 errors, 0 warnings)
npm run typecheck   # TypeScript check without emit (tsc --noEmit)
npm test            # Run 164 automated tests via native node:test
```

- **Run coverage:** `node --test --experimental-test-coverage "test/*.test.js"`
- **Run a single test:** `node --test test/api.test.js`
- **Dependencies audit:** `npm run audit` (fails on high/critical advisories).
- **Register slash commands:** `npm run deploy`
- *Note:* 12 tests in `binaries.test.js` and `relay.test.js` are intentionally skipped on Windows (POSIX shebang scripts).

---

## 2. Directory Architecture

```text
src/
├── index.js                      # Root entry point
├── config.js                     # Frozen configuration Object.freeze()
├── bot.js                        # Discord client & gateway event bindings
├── setup/
│   ├── bootstrap.js              # Safe bootloader (no bare process.exit)
│   ├── preflight.js              # Validation orchestrator (< 200L)
│   ├── preflight-checks.js       # Pure token, proxy and filesystem validators
│   ├── launcher.js               # ShardingManager & graceful shutdown handlers
│   ├── setup-error.js            # Structured SetupError hierarchy
│   └── report.js                 # Formatted stderr / CLI reporter
├── api/
│   ├── server.js                 # Express 5 initialization + Helmet + Rate limit
│   ├── contracts.js              # Zod schemas, formatSong/toFrontendTrack & RFC 7807
│   ├── shard-proxy.js            # Multi-shard broadcastEval router (30s cache)
│   ├── sse.js                    # Server-Sent Events hub by guildId
│   ├── middleware/
│   │   ├── auth.js               # timingSafeEqual Bearer check & X-User-Discord-Id
│   │   └── validate.js           # Zod validator (Snowflakes, SSRF guard, bodies)
│   ├── v1/
│   │   ├── router.js             # API v1 router (with legacy /api fallback)
│   │   └── routes/               # player, queue, discovery routes
│   ├── announce.js
│   └── tunnel.js                 # Cloudflare quick tunnel manager
├── music/
│   ├── manager.js                # Core facade (< 200L) with Dependency Injection
│   ├── playback.js               # play(), enqueue(), atomic withPlayLock(guildId)
│   ├── events.js                 # DisTube event listeners
│   ├── panel.js                  # Components V2 panel lifecycle (debounce 750ms)
│   ├── presence.js               # Voice channel listener tracker & auto-leave
│   ├── plugins.js                # YouTubePlugin with Innertube integration
│   ├── plugins-base.js           # Extractor base classes & stream builders
│   ├── plugins-generic.js        # Generic & direct link fallback extractors
│   ├── innertube.js              # Direct YouTube Innertube client (< 250L)
│   ├── innertube-util.js         # Innertube duration parsers & constants
│   ├── relay.js                  # Local HTTP audio stream relay (< 250L)
│   ├── ytdlp.js                  # Concurrency semaphore & error sanitizer
│   ├── session.js                # Queue serialization & atomic store
│   ├── sessions-restore.js       # Auto-restore active sessions on startup
│   ├── actions.js                # Domain playback actions (pause, skip, filters)
│   ├── filters.js                # Audio equalizer & DSP presets
│   └── errors.js                 # User error mapper & host hints
├── core/
│   ├── store.js                  # Atomic async JsonStore (.tmp + fsync + drain)
│   ├── cache.js                  # TTLCache with periodic sweep & jitter
│   ├── i18n.js                   # Async pre-warmed translations cache
│   ├── logger.js                 # Pino structured logger (never use console.log)
│   ├── supabase.js               # Supabase client & connection verifier
│   ├── youtube-guards.js         # Anti-bot detection & SSRF URL guards
│   ├── format.js                 # chunk() protected with RangeError, time helpers
│   ├── user.js                   # resolveRequester()
│   ├── constants.js              # Fallback covers, emojis, limits
│   ├── dns-fix.js                # Dual-stack IPv4/IPv6 resolver
│   └── binaries/                 # Probe, streaming downloader & installers
└── ui/
    ├── views.js                  # Player panel view (< 200L)
    └── auxiliary-views.js        # Queue, lyrics, search, help, playlists views
```

---

## 3. Strict Coding Directives

1. **Size Limit:** No file in `src/` may exceed **250 lines**. `src/music/manager.js` must remain **< 200 lines**.
2. **Preserved Core Modules:** Do NOT rewrite or re-architect the base of:
   - `src/music/actions.js`, `src/commands/_shared.js`, `src/handlers/guards.js`
   - `src/music/errors.js`, `src/music/ytdlp.js`, `src/music/relay.js`
3. **Zero Synchronous I/O in Hot Paths:** Never use `fs.readFileSync`, `fs.readdirSync`, or `zlib.inflateRawSync` inside HTTP handlers or Discord commands. Translations and exclusions must be kept in-memory.
4. **Fail-Fast Security:**
   - `API_SECRET` must be at least 32 characters in production.
   - Authentication must use `crypto.timingSafeEqual` over buffers.
   - Mutations (`POST`, `PUT`, `DELETE`) require the `X-User-Discord-Id` header.
5. **No `process.exit()`:** Throw `SetupError` (startup) or `ApiError` (API). Only `index.js` or `shard.js` terminate the process.
6. **Concurrency Protection:** All queue modifications must be serialized using `manager.withPlayLock(guildId, task)` to prevent DisTube internal race conditions.
7. **Pino Logging:** Use `createLogger('scope')`. Never use `console.log` in `src/`.
8. **Internationalization (i18n):** User-facing strings must exist in all 23 language files under `languages/*.json`. Adding a key in only one file will fail `npm run check`.
9. **Documentation & Spec:** Any change or addition to the REST API must be reflected in `openapi.yaml`.

---

## 4. Testing & Mocks Conventions

- Unit tests use Node's native runner: `const { test, describe, it } = require('node:test')`.
- Assertions use `node:assert/strict`.
- External services (Discord client, DisTube, Supabase) must be mocked using test doubles or `test/helpers.js`. Tests must be runnable completely offline.
