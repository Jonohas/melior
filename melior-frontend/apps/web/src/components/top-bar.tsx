import type { ReactNode } from 'react';

type TopBarProps = {
    /** What the page is, with an optional icon before it. */
    title: ReactNode;
    /** Details shown beside the title, for example a channel's topic. */
    subtitle?: ReactNode;
    /** Status and actions, right-aligned. */
    children?: ReactNode;
};

/** The bar above every page: where you are, and what is going on there. */
export const TopBar = ({ title, subtitle, children }: TopBarProps) => (
    <header className="flex min-h-12 shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-border bg-card px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
            <h1 className="flex items-center gap-1.5 text-sm font-semibold break-all">{title}</h1>
            {subtitle && (
                <p className="truncate border-l border-border pl-2 text-sm text-muted-foreground">{subtitle}</p>
            )}
        </div>
        {children && <div className="ml-auto flex flex-wrap items-center gap-2">{children}</div>}
    </header>
);
