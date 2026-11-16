/** Interprets the `relay` search param: `1`, `"1"`, `true` and `"true"` mean "force the TURN relay". */
export const parseForceRelay = (value: unknown): boolean =>
    value === true || value === 1 || value === '1' || value === 'true';
