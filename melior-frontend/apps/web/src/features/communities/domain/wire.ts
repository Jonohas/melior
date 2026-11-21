import { isObject, isString, type JsonObject } from '@/features/conference/domain';

export { isObject, isString, type JsonObject };

export const isWholeNumber = (value: unknown): value is number =>
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;

export const isBoolean = (value: unknown): value is boolean => typeof value === 'boolean';

/** Parses every entry; null when the value is not an array or any entry does not fit. */
export const parseList = <T>(raw: unknown, parseItem: (item: unknown) => T | null): T[] | null => {
    if (!Array.isArray(raw)) return null;
    const items = raw.map(parseItem);
    return items.every((item): item is T => item !== null) ? items : null;
};

/** One of the known wire names, or null. */
export const oneOf = <T extends string>(known: readonly T[], value: unknown): T | null =>
    known.find((entry) => entry === value) ?? null;

/** A string, a null, or (for an absent key) null. Anything else is `undefined`, meaning "does not fit". */
export const nullableStringOf = (value: unknown): string | null | undefined => {
    if (value === undefined || value === null) return null;
    return isString(value) ? value : undefined;
};

/** A whole number, a null, or (for an absent key) null. Anything else is `undefined`, meaning "does not fit". */
export const nullableWholeOf = (value: unknown): number | null | undefined => {
    if (value === undefined || value === null) return null;
    return isWholeNumber(value) ? value : undefined;
};
