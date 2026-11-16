#!/usr/bin/env bash
# Makes the locally running stack reachable from the tailnet, over HTTPS (browsers
# only allow camera and microphone access on HTTPS or localhost).
#
#   docker/tailscale.sh up       configure and print how to start the app
#   docker/tailscale.sh down     undo the Tailscale part and the .env changes
#   docker/tailscale.sh status
#
# `up` does four things:
#   1. `tailscale serve` on two HTTPS ports of this machine's tailnet name:
#        :8443  -> the frontend  (127.0.0.1:3000)
#        :10000 -> Keycloak      (127.0.0.1:8300)
#      Port 443 is left alone. Tailscale provides the certificates; HTTPS must be
#      enabled for the tailnet.
#   2. Writes the matching URLs into docker/.env (FRONTEND_URL, KEYCLOAK_PUBLIC_URL,
#      IDP_ISSUER, ALLOWED_HOSTS) and moves coturn to the tailnet address
#      (COTURN_BIND, TURN_EXTERNAL_IP, MELIOR_TURN_URLS), so it works as a
#      fallback for devices that cannot reach the SFU's UDP ports directly.
#   3. Restarts Keycloak and coturn with those settings.
#   4. Allows the new frontend origin on the Keycloak client.
#
# Media itself does not go through `tailscale serve`: browsers send it straight to
# this machine's tailnet address over UDP, which the SFU listens on like on every
# other interface.
#
# While this is active everyone uses the tailnet URL, this machine included:
# the session cookie and the login redirect belong to that origin, so
# http://localhost:3000 no longer logs in.
set -euo pipefail

FRONTEND_PORT_HTTPS=8443
KEYCLOAK_PORT_HTTPS=10000
MANAGED="FRONTEND_URL KEYCLOAK_PUBLIC_URL IDP_ISSUER ALLOWED_HOSTS COTURN_BIND TURN_EXTERNAL_IP MELIOR_TURN_URLS"

cd "$(dirname "$0")"
[[ -f .env ]] || { echo "docker/.env is missing; run docker/init-env.sh first" >&2; exit 1; }

tailnet() {
    tailscale status --json | python3 -c '
import json, sys
status = json.load(sys.stdin)
if status.get("BackendState") != "Running":
    sys.exit("tailscale is not running")
me = status["Self"]
name = me["DNSName"].rstrip(".")
if name not in (status.get("CertDomains") or []):
    sys.exit("HTTPS certificates are not enabled for this tailnet (admin console -> DNS -> HTTPS)")
ipv4 = next(ip for ip in me["TailscaleIPs"] if "." in ip)
print(name, ipv4)'
}

# Sets KEY=value in .env, replacing any existing (or commented-out) line.
set_env() {
    local key=$1 value=$2 tmp
    tmp=$(mktemp)
    grep -v -E "^#?${key}=" .env > "$tmp" || true
    printf '%s=%s\n' "$key" "$value" >> "$tmp"
    cat "$tmp" > .env && rm -f "$tmp"
}

unset_env() {
    local tmp
    tmp=$(mktemp)
    grep -v -E "^$1=" .env > "$tmp" || true
    cat "$tmp" > .env && rm -f "$tmp"
}

wait_for_keycloak() {
    local url=$1
    for _ in $(seq 1 60); do
        [[ $(curl -s -o /dev/null -w '%{http_code}' "$url/realms/melior/.well-known/openid-configuration") == 200 ]] && return 0
        sleep 2
    done
    echo "Keycloak did not answer at $url" >&2
    return 1
}

case ${1:-} in
up)
    read -r name ip < <(tailnet)
    frontend="https://$name:$FRONTEND_PORT_HTTPS"
    keycloak="https://$name:$KEYCLOAK_PORT_HTTPS"

    tailscale serve --bg --https=$FRONTEND_PORT_HTTPS http://127.0.0.1:3000 >/dev/null
    tailscale serve --bg --https=$KEYCLOAK_PORT_HTTPS http://127.0.0.1:8300 >/dev/null

    set_env FRONTEND_URL "$frontend"
    set_env KEYCLOAK_PUBLIC_URL "$keycloak"
    set_env IDP_ISSUER "$keycloak/realms/melior"
    set_env ALLOWED_HOSTS "$name"
    set_env COTURN_BIND "$ip"
    set_env TURN_EXTERNAL_IP "$ip"
    set_env MELIOR_TURN_URLS "turn:$ip:3478?transport=udp"

    docker compose up -d keycloak coturn
    wait_for_keycloak "$keycloak"
    ./keycloak-allow-origin.sh "$frontend"

    cat <<TEXT

Reachable from the tailnet:
  app       $frontend
  Keycloak  $keycloak

Now (re)start the server and the frontend with the new settings:

  cd melior-server && set -a && . ../docker/.env && set +a && \\
    DATABASE_URL="postgres://\$POSTGRES_USER:\$POSTGRES_PASSWORD@127.0.0.1:5432/\$POSTGRES_DB" \\
    MELIOR_TURN_SECRET="\$TURN_SECRET" cargo run
  cd melior-frontend && set -a && . ../docker/.env && set +a && bun dev
TEXT
    ;;
down)
    tailscale serve --https=$FRONTEND_PORT_HTTPS off >/dev/null 2>&1 || true
    tailscale serve --https=$KEYCLOAK_PORT_HTTPS off >/dev/null 2>&1 || true
    for key in $MANAGED; do unset_env "$key"; done
    docker compose up -d keycloak coturn
    echo "Back to localhost only. Restart the server and the frontend."
    ;;
status)
    tailscale serve status
    echo
    grep -E "^($(echo $MANAGED | tr ' ' '|'))=" .env || echo "(no tailnet settings in docker/.env)"
    ;;
*)
    sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
