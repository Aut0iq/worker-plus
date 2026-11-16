#!/usr/bin/env bash
# Backup Postgres + photos of the docker compose stack.
# Usage: scripts/backup.sh [dest_dir]   (default ./backups, keeps KEEP_DAYS=14 days)
# Cron:  0 3 * * * cd /opt/worker-plus && scripts/backup.sh /var/backups/prila >> /var/log/prila-backup.log 2>&1
set -euo pipefail
cd "$(dirname "$0")/.."
dest="${1:-./backups}"
keep="${KEEP_DAYS:-14}"
ts=$(date +%Y%m%d-%H%M%S)
mkdir -p "$dest"

docker compose exec -T db pg_dump -U prila -d prila -Fc > "$dest/db-$ts.dump.part"
mv "$dest/db-$ts.dump.part" "$dest/db-$ts.dump"
docker compose exec -T server tar -C /data -czf - photos > "$dest/photos-$ts.tar.gz.part"
mv "$dest/photos-$ts.tar.gz.part" "$dest/photos-$ts.tar.gz"

find "$dest" -type f \( -name 'db-*.dump' -o -name 'photos-*.tar.gz' \) -mtime +"$keep" -delete
echo "$(date -Is) backup ok: $dest/db-$ts.dump $dest/photos-$ts.tar.gz"
