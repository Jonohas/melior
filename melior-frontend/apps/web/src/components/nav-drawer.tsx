import { useRouterState } from '@tanstack/react-router';
import { Menu } from 'lucide-react';
import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui';
import { cn } from '@/lib/utils';

export type NavDrawer = {
    /** Whether the drawer lies over the page. Only meaningful below the `md` breakpoint; above it the columns are inline. */
    open: boolean;
    toggle: () => void;
    close: () => void;
};

/** `null` outside the authenticated shell, where there is no drawer to open. */
const NavDrawerContext = createContext<NavDrawer | null>(null);

/** The navigation drawer's state; `null` on a page without the shell. */
export const useNavDrawer = (): NavDrawer | null => useContext(NavDrawerContext);

/** Closes the drawer on Escape while it is open. */
const useCloseOnEscape = (open: boolean, close: () => void) => {
    useEffect(() => {
        if (!open) return;
        const onKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') close();
        };
        window.addEventListener('keydown', onKeyDown);
        return () => window.removeEventListener('keydown', onKeyDown);
    }, [open, close]);
};

/**
 * Holds whether the server rail and channel column lie over the page as a drawer, on a screen too narrow to show
 * them beside it. Picking a channel (any navigation) and Escape close it.
 */
export const NavDrawerProvider = ({ children }: { children: ReactNode }) => {
    const [open, setOpen] = useState(false);
    const pathname = useRouterState({ select: (state) => state.location.pathname });
    const close = useCallback(() => setOpen(false), []);
    const toggle = useCallback(() => setOpen((current) => !current), []);

    // biome-ignore lint/correctness/useExhaustiveDependencies: a navigation is what closes the drawer
    useEffect(() => {
        close();
    }, [pathname, close]);
    useCloseOnEscape(open, close);

    const value = useMemo(() => ({ open, toggle, close }), [open, toggle, close]);
    return <NavDrawerContext.Provider value={value}>{children}</NavDrawerContext.Provider>;
};

/** The classes that turn a shell column into a drawer panel on a narrow screen: hidden until opened, then over the page. */
export const navDrawerColumn = (open: boolean, position: 'rail' | 'beside-rail'): string =>
    cn(
        'max-md:fixed max-md:inset-y-0 max-md:z-50',
        position === 'rail' ? 'max-md:left-0' : 'max-md:left-rail',
        !open && 'max-md:hidden',
    );

/** The header's button that opens the drawer; nothing on a wide screen or outside the shell. */
export const NavDrawerButton = ({ className }: { className?: string }) => {
    const drawer = useNavDrawer();
    if (!drawer) return null;
    return (
        <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            className={cn('md:hidden', className)}
            aria-label="Open navigation"
            aria-expanded={drawer.open}
            onClick={drawer.toggle}
        >
            <Menu />
        </Button>
    );
};

/** Dims the page behind the open drawer; a tap on it closes the drawer. */
export const NavDrawerBackdrop = () => {
    const drawer = useNavDrawer();
    if (!drawer?.open) return null;
    return (
        <button
            type="button"
            aria-label="Close navigation"
            className="fixed inset-0 z-40 bg-black/50 md:hidden"
            onClick={drawer.close}
        />
    );
};
