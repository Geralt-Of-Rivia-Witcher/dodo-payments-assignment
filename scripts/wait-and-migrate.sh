#!/usr/bin/env sh
set -eu

echo "[invoice-api] waiting for postgres..."
until pg_isready -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; do
  sleep 1
done

echo "[invoice-api] postgres is ready, starting application (sqlx migrations run in Rust startup)"
exec /app/invoice-api
