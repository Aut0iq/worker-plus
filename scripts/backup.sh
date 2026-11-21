#!/usr/bin/env bash
# Backup Postgres + photos + file logs of the docker compose stack.
# Usage: scripts/backup.sh [dest_dir]   (default ./backups, keeps KEEP_DAYS=14 days)
# Cron:  0 3 * * * cd /opt/worker-plus && scripts/backup.sh /var/backups/prila >> /var/log/prila-backup.log 2>&1
set -euo pipefail
umask 077   # dumps contain password hashes and student photos
cd "$(dirname "$0")/.."
dest="${1:-./backups}"
keep="${KEEP_DAYS:-14}"
ts=$(date +%Y%m%d-%H%M%S)
mkdir -p "$dest"

docker compose exec -T db pg_dump -U prila -d prila -Fc > "$dest/db-$ts.dump.part"
mv "$dest/db-$ts.dump.part" "$dest/db-$ts.dump"
# logs/ holds audit.log and komendant.log (required by the spec), not only photos
docker compose exec -T server tar -C /data -czf - photos logs > "$dest/photos-$ts.tar.gz.part"
mv "$dest/photos-$ts.tar.gz.part" "$dest/photos-$ts.tar.gz"

find "$dest" -type f \( -name 'db-*.dump' -o -name 'photos-*.tar.gz' \) -mtime +"$keep" -delete
echo "$(date -Is) backup ok: $dest/db-$ts.dump $dest/photos-$ts.tar.gz"
