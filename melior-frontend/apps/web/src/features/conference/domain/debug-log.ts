/** How many entries the debug log keeps; older ones are dropped. */
export const DEBUG_LOG_LIMIT = 50;

/** Received from the server, sent to it, or seen by this browser itself (a connection state change). */
export type DebugDirection = 'in' | 'out' | 'local';

/** One line of the debug log. `seq` counts every line ever logged; `at` is milliseconds since the Unix epoch. */
export type DebugLogEntry = { seq: number; at: number; direction: DebugDirection; text: string };

/** The log with one more entry, cut to the last `DEBUG_LOG_LIMIT`. */
export const appendLog = (
    log: readonly DebugLogEntry[],
    entry: DebugLogEntry,
    limit = DEBUG_LOG_LIMIT,
): DebugLogEntry[] => [...log, entry].slice(-limit);

/** A duration in seconds with one decimal, e.g. `10.0 s`. */
export const formatSeconds = (ms: number): string => `${(Math.max(0, ms) / 1000).toFixed(1)} s`;
