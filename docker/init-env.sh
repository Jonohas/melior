#!/usr/bin/env bash
# Creates docker/.env from .env.example with freshly generated random secrets.
#
#   ./init-env.sh [--force] [--output PATH]
#
# Replaces POSTGRES_PASSWORD, KEYCLOAK_DB_PASSWORD, KEYCLOAK_ADMIN_PASSWORD,
# TURN_SECRET and RUSTFS_SECRET_KEY. Refuses to overwrite an existing file unless --force is given.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
example="$here/.env.example"
output="$here/.env"
force=0

while [ $# -gt 0 ]; do
    case "$1" in
        --force) force=1 ;;
        --output)
            [ $# -ge 2 ] || { echo "--output needs a path" >&2; exit 2; }
            output="$2"
            shift
            ;;
        -h | --help)
            sed -n '2,8p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

if [ -e "$output" ] && [ "$force" -ne 1 ]; then
    echo "$output already exists; use --force to overwrite it." >&2
    exit 1
fi

random_secret() {
    if command -v openssl >/dev/null 2>&1; then
        openssl rand -hex 24
    else
        head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n'
    fi
}

umask 077
content="$(cat "$example")"
for name in POSTGRES_PASSWORD KEYCLOAK_DB_PASSWORD KEYCLOAK_ADMIN_PASSWORD TURN_SECRET RUSTFS_SECRET_KEY; do
    secret="$(random_secret)"
    if ! grep -q "^${name}=" <<<"$content"; then
        echo "$name is missing from $example" >&2
        exit 1
    fi
    content="$(sed "s|^${name}=.*|${name}=${secret}|" <<<"$content")"
done

printf '%s\n' "$content" >"$output"
chmod 600 "$output"

cat <<EOF
Wrote $output (mode 600) with new random values for:
  POSTGRES_PASSWORD, KEYCLOAK_DB_PASSWORD, KEYCLOAK_ADMIN_PASSWORD, TURN_SECRET,
  RUSTFS_SECRET_KEY
Everything else is copied from .env.example.

Postgres reads its passwords only when its data volume is first created. If the
volume already exists, changing POSTGRES_PASSWORD or KEYCLOAK_DB_PASSWORD makes
the stack fail to connect; run 'docker compose down -v' (this deletes the
database and the Keycloak realm) before starting with the new file.
EOF
