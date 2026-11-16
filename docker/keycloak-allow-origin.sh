#!/usr/bin/env bash
# Allows the "melior" client in the running Keycloak to be used from one more
# origin: adds <origin>/* to its redirect URIs and post-logout redirect URIs,
# and <origin> to its web origins. Nothing is removed, and running it twice
# changes nothing.
#
#   docker/keycloak-allow-origin.sh https://melior.example.org
#
# The realm file is imported only when the realm does not exist yet, so an
# origin added to docker/keycloak/melior-realm.json later is not picked up by a
# running stack. This script changes the running one. Needs python3.
set -euo pipefail

origin=${1:?usage: keycloak-allow-origin.sh <origin, e.g. https://host.example.org:8443>}
origin=${origin%/}
case $origin in
    http://*|https://*) ;;
    *) echo "origin must start with http:// or https://" >&2; exit 2 ;;
esac

cd "$(dirname "$0")"
[[ -f .env ]] || { echo "docker/.env is missing; run docker/init-env.sh first" >&2; exit 1; }
set -a; . ./.env; set +a

kcadm() { docker compose exec -T keycloak /opt/keycloak/bin/kcadm.sh "$@"; }

kcadm config credentials --server http://localhost:8080 --realm master \
    --user "${KEYCLOAK_ADMIN_USERNAME:-admin}" --password "$KEYCLOAK_ADMIN_PASSWORD" >/dev/null

id=$(kcadm get clients -r melior -q clientId=melior --fields id --format csv --noquotes | tr -d '\r')
[[ -n $id ]] || { echo 'client "melior" not found in realm "melior"' >&2; exit 1; }

kcadm get "clients/$id" -r melior | ORIGIN=$origin python3 -c '
import json, os, sys

origin = os.environ["ORIGIN"]
client = json.load(sys.stdin)

def add(values, value):
    return values if value in values else values + [value]

client["redirectUris"] = add(client.get("redirectUris", []), origin + "/*")
client["webOrigins"] = add(client.get("webOrigins", []), origin)
attributes = client.setdefault("attributes", {})
logout = [uri for uri in attributes.get("post.logout.redirect.uris", "").split("##") if uri]
attributes["post.logout.redirect.uris"] = "##".join(add(logout, origin + "/*"))
json.dump(client, sys.stdout)
' | kcadm update "clients/$id" -r melior -f -

echo "client \"melior\" now accepts $origin"
