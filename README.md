# Rust Athena

A Rust server for the [Rodecomp client](https://github.com/rodecomp/rodecomp), using the client protocol for `PACKETVER=20260219`. Login, character, and map admission run as separate processes. Accounts and characters persist in MariaDB through the MySQL driver. The map process admits a transferred character and starts loading Prontera at a fixed spawn position.

## Local setup

Install Rust, Docker Compose, and the MySQL client development library used by Diesel. On macOS with Homebrew's keg-only MySQL client, add it to the linker search path before building.

```sh
export LIBRARY_PATH="$(brew --prefix mysql-client)/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
```

Start the local database and configure both server processes to use it. Export this URL in each terminal that runs a server, or put it in a local `.env` file.

```sh
docker compose up -d db
export DATABASE_URL='mysql://ragnarok:ragnarok@127.0.0.1:3306/ragnarok'
```

On a fresh database, apply all four migrations in this order after MariaDB finishes initializing. If the original `users` migration is already applied, start with the second migration. It requires unique usernames under the database collation. Review existing duplicates before running it. Old password values must already be bcrypt hashes to work with the login server. For an existing database, apply only the migrations you have yet to run. If you already run the map server, apply the last migration before starting the updated processes.

```sh
docker compose exec -T db mariadb -uragnarok -pragnarok ragnarok \
  < db/migrations/2022-04-09-114552_create_users/up.sql
docker compose exec -T db mariadb -uragnarok -pragnarok ragnarok \
  < db/migrations/2026-11-23-000000_game_servers/up.sql
docker compose exec -T db mariadb -uragnarok -pragnarok ragnarok \
  < db/migrations/2026-11-23-000001_map_transfers/up.sql
docker compose exec -T db mariadb -uragnarok -pragnarok ragnarok \
  < db/migrations/2026-11-23-000002_character_positions/up.sql
cargo build --locked
```

Register a local account using the CLI. In Bash or Zsh, the following reads the password without echo and passes it on standard input.

```sh
read -rs athena_password
printf '%s\n' "$athena_password" | target/debug/rust-athena register myuser me@example.invalid
unset athena_password
```

Start each process in its own terminal.

```sh
target/debug/rust-athena map
target/debug/rust-athena char
target/debug/rust-athena login
```

Point Rodecomp at `127.0.0.1:6900`, sign in, select **Rust Athena**, and create a character in an empty slot. Sign out and back in to check that it reloads. The current creation rules accept Novice and Summoner, and names of 4–23 ASCII letters, digits, or underscores.

To validate map loading, select a saved character. Rodecomp should load Prontera at `(156, 191)` and the map process should log `Character … loaded prontera.gat at (156, 191)`. The map process handles initial loading and keeps the connection alive.

## Try movement in Prontera

Build the current binary and start the map process with an rAthena renewal map cache that matches your client’s terrain. For a local rAthena checkout, use its `db/re/map_cache.dat`. Keep the cache outside this repository.

```sh
ATHENA_MAP_CACHE=/path/to/rathena/db/re/map_cache.dat target/debug/rust-athena map
```

When the server logs `Loaded walkability for prontera.gat`, connect and select a character. Click a nearby open tile, let the character arrive, and click again. Each accepted move logs its start and destination. Movement is limited to a route of up to 14 tiles. Walls, out-of-bounds destinations, and clicks while a walk is active are ignored. After disconnecting, reconnect to resume at the last tile reached in Prontera. Characters without a usable saved position start at (156, 191).

## Configuration

| Variable | Default | Used by |
| --- | --- | --- |
| `DATABASE_URL` | Required | Registration and all server roles |
| `ATHENA_PACKETVER` | `20260219` | Server startup |
| `ATHENA_LOGIN_BIND` | `127.0.0.1:6900` | Login |
| `ATHENA_CHAR_BIND` | `127.0.0.1:6121` | Character |
| `ATHENA_CHAR_ADVERTISE` | `127.0.0.1:6121` | Login reply sent to the client |
| `ATHENA_MAP_CACHE` | Optional. Movement is disabled when unset. | Map |
| `ATHENA_MAP_BIND` | `127.0.0.1:5121` | Map |
| `ATHENA_MAP_ADVERTISE` | `127.0.0.1:5121` | Character reply sent to the client |
| `ATHENA_CHAR_NAME` | `Rust Athena` | Login reply sent to the client |

Keep development instances on loopback or a trusted private network. The supported client login packet carries credentials without transport encryption.

Run `cargo test --locked --workspace` for the protocol regressions. See the [architecture notes](docs/architecture.md) for ownership, validation, and the map-server implementation sequence.
