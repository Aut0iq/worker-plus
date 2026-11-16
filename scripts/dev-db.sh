#!/usr/bin/env bash
# Manage the local dev Postgres (zonky embedded binaries, no root/docker needed).
# Usage: scripts/dev-db.sh {install|start|stop|status}
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# PG_HOME must live on a Linux fs: initdb refuses drvfs (/mnt/c) permissions.
PG_HOME="${PG_HOME:-$HOME/.devdb}"
PG_BIN="$PG_HOME/bin"
PG_DATA="$PG_HOME/data"
PG_PORT="${PG_PORT:-5433}"
PG_USER="prila"
PG_DB="prila"
ZONKY_VERSION="16.15.0"
ZONKY_JAR_URL="https://repo1.maven.org/maven2/io/zonky/test/postgres/embedded-postgres-binaries-linux-amd64/${ZONKY_VERSION}/embedded-postgres-binaries-linux-amd64-${ZONKY_VERSION}.jar"

case "${1:-}" in
install)
    mkdir -p "$PG_HOME"
    jar="$PG_HOME/pg.jar"
    if [ ! -f "$jar" ]; then
        echo "downloading $ZONKY_JAR_URL"
        if command -v curl >/dev/null; then
            curl -fsSL "$ZONKY_JAR_URL" -o "$jar"
        else
            python3 -c "import urllib.request,sys; urllib.request.urlretrieve(sys.argv[1], sys.argv[2])" "$ZONKY_JAR_URL" "$jar"
        fi
    fi
    tmp="$(mktemp -d)"
    if command -v unzip >/dev/null; then
        unzip -o -q "$jar" -d "$tmp"
    else
        python3 -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$jar" "$tmp"
    fi
    txz="$(find "$tmp" -name '*.txz' | head -1)"
    tar -xJf "$txz" -C "$PG_HOME"
    rm -rf "$tmp" "$jar"
    echo "installed postgres $ZONKY_VERSION to $PG_HOME"
    ;;
start)
    if [ ! -d "$PG_DATA" ]; then
        "$PG_BIN/initdb" -D "$PG_DATA" -U "$PG_USER" --encoding=UTF8 \
            --auth=trust --locale=C --no-instructions
        echo "unix_socket_directories = '$PG_DATA'" >> "$PG_DATA/postgresql.conf"
        # the zonky bundle has no psql — create the app db in single-user mode
        echo "CREATE DATABASE $PG_DB" \
            | "$PG_BIN/postgres" --single -D "$PG_DATA" postgres >/dev/null 2>&1 || true
    fi
    "$PG_BIN/pg_ctl" -D "$PG_DATA" -l "$PG_HOME/postgres.log" -o "-p $PG_PORT" start
    sleep 1
    if ! "$PG_BIN/pg_ctl" -D "$PG_DATA" status >/dev/null 2>&1; then
        echo "pg did not start — see $PG_HOME/postgres.log" >&2
        exit 1
    fi
    echo "postgres $ZONKY_VERSION on 127.0.0.1:$PG_PORT db=$PG_DB user=$PG_USER (trust auth)"
    echo "DATABASE_URL=postgres://$PG_USER@127.0.0.1:$PG_PORT/$PG_DB"
    ;;
stop)
    "$PG_BIN/pg_ctl" -D "$PG_DATA" -m fast stop
    ;;
status)
    "$PG_BIN/pg_ctl" -D "$PG_DATA" status
    ;;
*)
    echo "usage: $0 {install|start|stop|status}" >&2
    exit 2
    ;;
esac
