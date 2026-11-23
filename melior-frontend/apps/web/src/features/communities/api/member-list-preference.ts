import { initialMemberListOpen, parseMemberListOpen } from '@/features/communities/domain';
import { screenAtLeast } from './screen';

const MEMBER_LIST_KEY = 'melior.member-list';
/** Below this width the member list would squeeze the channel, so it starts closed: `--breakpoint-wide` in `styles.css`. */
const WIDE_SCREEN_MIN_WIDTH = '1100px';

const storedMemberListOpen = (): boolean | null => {
    try {
        return parseMemberListOpen(window.localStorage.getItem(MEMBER_LIST_KEY));
    } catch {
        return null;
    }
};

/** Whether the member list starts open: as the person left it, else on a wide screen only. */
export const loadMemberListOpen = (): boolean =>
    initialMemberListOpen(storedMemberListOpen(), screenAtLeast(WIDE_SCREEN_MIN_WIDTH));

/** Remembers whether the member list is open, like the call's microphone and camera switches. */
export const saveMemberListOpen = (open: boolean): void => {
    try {
        window.localStorage.setItem(MEMBER_LIST_KEY, String(open));
    } catch {
        // Private browsing or a full quota: the choice still applies to this page.
    }
};
