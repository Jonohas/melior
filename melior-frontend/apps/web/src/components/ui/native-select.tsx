import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

/** The browser's own select, styled like an input: keyboard and screen-reader support come for free. */
function NativeSelect({ className, ...props }: ComponentProps<'select'>) {
    return (
        <select
            data-slot="native-select"
            className={cn(
                'h-9 w-full rounded-md border border-input bg-secondary px-3 text-sm shadow-(--input-shadow) outline-none',
                'focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:opacity-50',
                '[color-scheme:dark]',
                className,
            )}
            {...props}
        />
    );
}

export { NativeSelect };
