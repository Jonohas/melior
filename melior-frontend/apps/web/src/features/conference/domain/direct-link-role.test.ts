import { expect, test } from 'vitest';
import { isOfferer } from './direct-link-role';

test('exactly one side offers: the lower session id', () => {
    expect(isOfferer('a', 'b')).toBe(true);
    expect(isOfferer('b', 'a')).toBe(false);
    expect(isOfferer('a', 'a')).toBe(false);
    expect(isOfferer('B', 'a')).toBe(true);
});
