#!/usr/bin/env bash
# Starts one release of Melior on its VM:
#
#   deploy.sh X.Y.Z [GITHUB-USER < TOKEN]
#
# .github/workflows/deploy.yml copies this script and compose.yaml next to .env, which
# Jonohas/infra writes, and runs it as `deploy`. A GitHub token on stdin pulls the images and is
# never stored; without one the images must be public. Deploying an older version rolls back to it.
set -euo pipefail

dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
version=${1:-}
registry_user=${2:-}
semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'

if [[ ! $version =~ $semver ]]; then
    echo "usage: ${0##*/} X.Y.Z [GITHUB-USER < TOKEN]" >&2
    exit 2
fi

cd "$dir"

# One deploy at a time; the lock is released when the script exits.
exec 9>"$dir/.deploy.lock"
if ! flock --nonblock 9; then
    echo "another deploy is running" >&2
    exit 1
fi

for file in compose.yaml .env; do
    if [[ ! -f $file ]]; then
        echo "missing $dir/$file" >&2
        exit 1
    fi
done

# The login goes to a throwaway Docker config, so the token never lands on disk for good.
token=""
if [[ ! -t 0 ]]; then
    token=$(cat)
fi
if [[ -n $token && -n $registry_user ]]; then
    DOCKER_CONFIG=$(mktemp -d)
    export DOCKER_CONFIG
    trap 'rm -rf "$DOCKER_CONFIG"' EXIT
    docker login ghcr.io --username "$registry_user" --password-stdin <<<"$token" >/dev/null
fi

printf 'MELIOR_VERSION=%s\n' "$version" >version.env

compose=(docker compose --env-file .env --env-file version.env -f compose.yaml)
"${compose[@]}" pull
"${compose[@]}" up -d --wait --remove-orphans

echo "Deployed Melior $version"
