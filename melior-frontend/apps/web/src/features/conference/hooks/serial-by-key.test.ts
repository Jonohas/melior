import { describe, expect, test } from 'vitest';
import { createSerialByKey } from './serial-by-key';

const deferred = () => {
    let resolve: () => void = () => undefined;
    const promise = new Promise<void>((done) => {
        resolve = done;
    });
    return { promise, resolve };
};

describe('createSerialByKey', () => {
    test('jobs of one key run one after another, in order', async () => {
        const serial = createSerialByKey<string>();
        const log: string[] = [];
        const gate = deferred();
        const first = serial('camera', async () => {
            log.push('first start');
            await gate.promise;
            log.push('first end');
        });
        const second = serial('camera', async () => {
            log.push('second start');
        });
        await Promise.resolve();
        expect(log).toEqual(['first start']);
        gate.resolve();
        await Promise.all([first, second]);
        expect(log).toEqual(['first start', 'first end', 'second start']);
    });

    test('jobs of another key do not wait', async () => {
        const serial = createSerialByKey<string>();
        const gate = deferred();
        const slow = serial('camera', () => gate.promise);
        await serial('microphone', async () => undefined);
        gate.resolve();
        await slow;
    });

    test('a failed job rejects its caller and the next one still runs', async () => {
        const serial = createSerialByKey<string>();
        const failed = serial('camera', async () => {
            throw new Error('no device');
        });
        const next = serial('camera', async () => 'ok');
        await expect(failed).rejects.toThrow('no device');
        await expect(next).resolves.toBe('ok');
    });
});
