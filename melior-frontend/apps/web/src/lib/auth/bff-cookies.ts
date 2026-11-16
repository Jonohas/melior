/** Readable marker the BFF sets (Path=/, 30 s) after a completed token exchange. */
export const LOGIN_COMPLETED_COOKIE = 'bff_login_completed';

const readCookie = (name: string): string | null => {
    if (typeof document === 'undefined') return null;
    const prefix = `${name}=`;
    for (const part of document.cookie.split(';')) {
        const trimmed = part.trim();
        if (trimmed.startsWith(prefix)) return decodeURIComponent(trimmed.slice(prefix.length));
    }
    return null;
};

/** True when the BFF marked a completed token exchange on the redirect that landed us here. */
export const hasLoginCompleted = (): boolean => readCookie(LOGIN_COMPLETED_COOKIE) !== null;

/** Expires the marker. The gate consumes it so the 30 s Max-Age cannot outlive the decision it informs. */
export const clearLoginCompleted = (): void => {
    if (typeof document === 'undefined') return;
    document.cookie = `${LOGIN_COMPLETED_COOKIE}=; path=/; max-age=0; samesite=lax`;
};
