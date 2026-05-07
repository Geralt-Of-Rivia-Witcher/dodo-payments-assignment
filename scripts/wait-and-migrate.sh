#!/usr/bin/env sh
set -eu

echo "[invoice-api] waiting for postgres..."
until pg_isready -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; do
  sleep 1
done

echo "[invoice-api] postgres is ready, running migrations..."
for f in /app/migrations/*.sql; do
  ver=$(basename "$f")
  applied=$(psql "postgresql://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME" -tAc "SELECT 1 FROM schema_migrations WHERE version='${ver}'")
  if [ "$applied" = "1" ]; then
    echo "[invoice-api] skipping $ver (already applied)"
    continue
  fi

  echo "[invoice-api] applying $ver"
  psql "postgresql://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME" -v ON_ERROR_STOP=1 -f "$f"
  psql "postgresql://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME" -v ON_ERROR_STOP=1 -c "INSERT INTO schema_migrations(version) VALUES ('${ver}')"
done

echo "[invoice-api] starting application"
exec /app/invoice-api
