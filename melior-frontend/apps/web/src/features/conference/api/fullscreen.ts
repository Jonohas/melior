/** Whether this browser can show an element fullscreen (iOS Safari on a phone cannot). */
export const canFullscreen = (): boolean => typeof document !== 'undefined' && document.fullscreenEnabled === true;

/** Whether `element` is the one shown fullscreen right now. */
export const isFullscreen = (element: Element | null): boolean =>
    element !== null && document.fullscreenElement === element;

/**
 * Shows `element` fullscreen, or leaves fullscreen when it already is. A refusal (no user gesture, a
 * policy) is logged, never left unhandled.
 */
export const toggleFullscreen = (element: Element): void => {
    const change = isFullscreen(element) ? document.exitFullscreen() : element.requestFullscreen();
    change.catch((error) => console.warn('fullscreen', error));
};

/** Calls `onChange` when any element enters or leaves fullscreen. Returns the function that stops listening. */
export const watchFullscreen = (onChange: () => void): (() => void) => {
    document.addEventListener('fullscreenchange', onChange);
    return () => document.removeEventListener('fullscreenchange', onChange);
};
