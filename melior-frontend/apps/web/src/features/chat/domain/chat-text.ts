/** The longest message the server accepts, in characters (not bytes). */
export const MAX_CHAT_CHARS = 2000;

export type DraftCheck = { ok: true; text: string } | { ok: false; reason: 'empty' | 'too_long' | 'invalid_character' };

/**
 * Checks a message the way the server does, so the composer can say what is wrong before anything is sent:
 * trimmed, at most `MAX_CHAT_CHARS` characters, and no control characters except line breaks and tabs. The text may
 * be empty only when the message carries an attachment.
 */
export const checkDraft = (draft: string, withAttachment = false): DraftCheck => {
    const text = draft.trim();
    if (text === '') return withAttachment ? { ok: true, text } : { ok: false, reason: 'empty' };
    if (Array.from(text).length > MAX_CHAT_CHARS) return { ok: false, reason: 'too_long' };
    if (/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/.test(text)) return { ok: false, reason: 'invalid_character' };
    return { ok: true, text };
};
