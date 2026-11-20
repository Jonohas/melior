import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { ReactNode } from 'react';

type TopBarProps = {
    /** The page's own name, shown after "Rooms". Omit on the lobby itself. */
    room?: string;
    /** Status and details about the page, right-aligned. */
    children?: ReactNode;
};

/** The bar above every page: where you are, and what is going on there. */
export const TopBar = ({ room, children }: TopBarProps) => (
    <header className="flex min-h-12 shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-border bg-card px-4 py-2">
        <nav aria-label="Breadcrumb" className="flex items-center gap-1.5 text-sm">
            {room ? (
                <>
                    <Link to="/" className="text-muted-foreground hover:text-foreground">
                        Rooms
                    </Link>
                    <ChevronRight className="size-3.5 text-muted-foreground" aria-hidden="true" />
                    <span className="font-medium break-all" aria-current="page">
                        {room}
                    </span>
                </>
            ) : (
                <span className="font-medium">Rooms</span>
            )}
        </nav>
        {children && <div className="ml-auto flex flex-wrap items-center gap-2">{children}</div>}
    </header>
);
