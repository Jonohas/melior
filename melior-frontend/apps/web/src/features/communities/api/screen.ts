/** Whether the screen is at least `minWidth` wide; false where there is no window (tests, prerender). */
export const screenAtLeast = (minWidth: string): boolean =>
    typeof window !== 'undefined' && window.matchMedia(`(min-width: ${minWidth})`).matches;
