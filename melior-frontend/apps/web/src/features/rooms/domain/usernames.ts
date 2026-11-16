export const USERNAME_PATTERN = /^[a-z0-9._@-]{1,64}$/;

/** Lower-cases and trims a username. Null when it is not 1 to 64 characters of `a-z0-9._@-`. */
export const normalizeUsername = (raw: string): string | null => {
    const username = raw.trim().toLowerCase();
    return USERNAME_PATTERN.test(username) ? username : null;
};

export type AddMemberResult = { ok: true; members: string[] } | { ok: false; error: string };

/** Adds an invited username to the list: normalised, validated, without duplicates. */
export const addMember = (members: readonly string[], raw: string): AddMemberResult => {
    if (raw.trim() === '') return { ok: false, error: 'Enter a username.' };
    const username = normalizeUsername(raw);
    if (!username)
        return { ok: false, error: 'Use 1 to 64 characters: letters, digits, dot, underscore, at sign or dash.' };
    if (members.includes(username)) return { ok: false, error: `${username} is already invited.` };
    return { ok: true, members: [...members, username] };
};

export const removeMember = (members: readonly string[], username: string): string[] =>
    members.filter((member) => member !== username);
