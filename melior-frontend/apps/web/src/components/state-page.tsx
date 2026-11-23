import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { Card, CardContent } from '@/components/ui';
import { cn } from '@/lib/utils';
import { NavDrawerButton } from './nav-drawer';

type StatePageProps = {
    icon: LucideIcon;
    title: string;
    /** Role `alert` for a page that says something went wrong. */
    alert?: boolean;
    /** Outside the app shell: takes the whole screen instead of the space beside the sidebar. */
    standalone?: boolean;
    children?: ReactNode;
    actions?: ReactNode;
};

/** A full-page message in the middle of the screen: a refusal, an error, a missing page. */
export const StatePage = ({
    icon: Icon,
    title,
    alert = false,
    standalone = false,
    children,
    actions,
}: StatePageProps) => (
    <main
        className={cn('relative grid min-h-0 flex-1 place-items-center p-4 sm:p-6', standalone && 'min-h-dvh')}
        role={alert ? 'alert' : undefined}
    >
        {!standalone && <NavDrawerButton className="absolute top-2 left-2" />}
        <Card className="w-full max-w-md">
            <CardContent className="flex flex-col items-center gap-3 py-8 text-center">
                <span className="grid size-12 place-items-center rounded-full bg-muted">
                    <Icon className="size-5" aria-hidden="true" />
                </span>
                <h1 className="text-xl font-semibold">{title}</h1>
                {children && <div className="text-sm text-muted-foreground">{children}</div>}
                {actions && <div className="mt-1 flex flex-wrap items-center justify-center gap-2">{actions}</div>}
            </CardContent>
        </Card>
    </main>
);
