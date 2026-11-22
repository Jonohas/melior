/**
 * Interprets an on/off search param such as `relay` (force the TURN relay) or `debug` (show the call's debug
 * panel): `1`, `"1"`, `true` and `"true"` mean on, everything else off.
 */
export const parseSearchFlag = (value: unknown): boolean =>
    value === true || value === 1 || value === '1' || value === 'true';
