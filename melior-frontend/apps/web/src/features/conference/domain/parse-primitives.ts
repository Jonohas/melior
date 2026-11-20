import type { IceCandidate } from './signalling-message';

export type JsonObject = Record<string, unknown>;

export const isObject = (value: unknown): value is JsonObject =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

export const isString = (value: unknown): value is string => typeof value === 'string';

export const optionalString = (value: unknown): value is string | undefined => value === undefined || isString(value);

export const nullableString = (value: unknown): value is string | null | undefined =>
    value === undefined || value === null || isString(value);

export const nullableNumber = (value: unknown): value is number | null | undefined =>
    value === undefined || value === null || (typeof value === 'number' && Number.isFinite(value));

export const parseIceCandidate = (value: unknown): IceCandidate | null => {
    if (!isObject(value)) return null;
    const { candidate, sdpMid, sdpMLineIndex, usernameFragment } = value;
    if (!optionalString(candidate) || !nullableString(sdpMid) || !nullableNumber(sdpMLineIndex)) return null;
    if (!nullableString(usernameFragment)) return null;
    return { candidate, sdpMid, sdpMLineIndex, usernameFragment };
};
