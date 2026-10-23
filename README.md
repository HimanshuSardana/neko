# Neko 🐾

Self-hosted media server for **anime episodes, manga chapters and books**.
Rust + Axum + SQLite. Single binary, no external DB required.

## Features (v1 roadmap)

- [x] Project scaffold + health endpoint
- [ ] SQLite schema (users, libraries, anime, manga, books, progress)
- [ ] Auth (register/login, Argon2 + JWT, admin/user roles)
- [ ] Library catalog APIs (CRUD libraries, list series/episodes/chapters/books)
- [ ] Streaming (HTTP Range for video, page/file serving for manga/books) + progress tracking + library scan

## Quickstart

```bash
cp .env.example .env
cargo run
# GET http://127.0.0.1:3000/health
```

## Config

| Var | Default | Description |
|-----|---------|-------------|
| `DATABASE_URL` | `sqlite://neko.db?mode=rwc` | SQLite connection string |
| `JWT_SECRET` | `change-me` | HS256 signing secret (min 32 chars in prod) |
| `JWT_EXPIRY_HOURS` | `168` | Token lifetime (7d) |
| `BIND_ADDR` | `127.0.0.1:3000` | Listen address |
| `MEDIA_ROOT` | `./media` | Root dir for `media/anime`, `media/manga`, `media/books` |

## Layout

```
src/
  main.rs        — wiring, router, state
  config.rs      — env config
  db.rs          — sqlite pool + migrations
  models.rs      — serde structs
  auth.rs        — password hashing, JWT, middleware
  routes/        — auth, libraries, anime, manga, books, stream, progress, scan
migrations/      — versioned SQL (sqlx)
media/           — (gitignored) drop files here
```

## API (v1)

```
GET  /health
POST /api/auth/register  {username, email, password}
POST /api/auth/login     {username, password} -> {token}
GET  /api/auth/me        (Bearer)

GET/POST          /api/libraries            (admin: POST/DELETE)
GET               /api/anime                ?library_id=&q=
GET               /api/anime/:id
GET               /api/anime/:id/episodes
GET               /api/stream/episode/:id   (Range support)

GET               /api/manga
GET               /api/manga/:id
GET               /api/manga/:id/chapters
GET               /api/stream/chapter/:id/:page

GET               /api/books
GET               /api/books/:id
GET               /api/stream/book/:id

GET/PUT           /api/progress            (Bearer)
POST              /api/scan/:library_id    (admin)
```

## Docker

```bash
docker build -t neko .
docker run -p 3000:3000 -v ./media:/app/media -v ./data:/app/data neko
```

See `Dockerfile` and `docker-compose.yml` (added with streaming feature).
