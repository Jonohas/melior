import { type RefObject, useCallback, useEffect, useState } from 'react';
import { canFullscreen, isFullscreen, toggleFullscreen, watchFullscreen } from '@/features/conference/api';

export type Fullscreen = {
    /** Whether this browser can show the element fullscreen at all. */
    supported: boolean;
    /** Whether the element is fullscreen right now. */
    active: boolean;
    /** Enters fullscreen, or leaves it when the element already is. */
    toggle: () => void;
};

/** Fullscreen for one element, such as a shared screen or a camera tile. Escape leaves it, as everywhere. */
export const useFullscreen = (ref: RefObject<HTMLElement | null>): Fullscreen => {
    const [active, setActive] = useState(false);

    useEffect(() => watchFullscreen(() => setActive(isFullscreen(ref.current))), [ref]);

    const toggle = useCallback(() => {
        if (ref.current) toggleFullscreen(ref.current);
    }, [ref]);

    return { supported: canFullscreen(), active, toggle };
};
