#!/bin/sh
# Creates Keycloak's own database and role next to the application database.
# Runs once, when the postgres data volume is first initialised.
set -eu

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
    -v kc_user="$KEYCLOAK_DB_USER" -v kc_password="$KEYCLOAK_DB_PASSWORD" -v kc_db="$KEYCLOAK_DB_NAME" <<'SQL'
CREATE ROLE :"kc_user" LOGIN PASSWORD :'kc_password';
CREATE DATABASE :"kc_db" OWNER :"kc_user";
SQL
