/**
 * Runs the jobs of one key one after another; jobs of different keys do not wait for each other. A job that
 * fails rejects its own caller and does not hold up the next one.
 */
export const createSerialByKey = <K extends string>() => {
    const tails = new Map<K, Promise<void>>();

    return <T>(key: K, job: () => Promise<T>): Promise<T> => {
        const run = (tails.get(key) ?? Promise.resolve()).then(job);
        tails.set(
            key,
            run.then(
                () => undefined,
                () => undefined,
            ),
        );
        return run;
    };
};
