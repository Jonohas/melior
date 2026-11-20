export const NAME_MAX_CHARS = 100;
export const TOPIC_MAX_CHARS = 1024;

export type TextCheck = { ok: true; value: string } | { ok: false; error: string };

const length = (value: string): number => Array.from(value).length;

/** A server, role, category or channel name: trimmed, 1 to 100 characters. */
export const checkName = (raw: string): TextCheck => {
    const value = raw.trim();
    if (value === '') return { ok: false, error: 'Enter a name.' };
    if (length(value) > NAME_MAX_CHARS) return { ok: false, error: `A name is at most ${NAME_MAX_CHARS} characters.` };
    return { ok: true, value };
};

/** A channel topic: trimmed, at most 1024 characters, may be empty. */
export const checkTopic = (raw: string): TextCheck => {
    const value = raw.trim();
    if (length(value) > TOPIC_MAX_CHARS) {
        return { ok: false, error: `A topic is at most ${TOPIC_MAX_CHARS} characters.` };
    }
    return { ok: true, value };
};
