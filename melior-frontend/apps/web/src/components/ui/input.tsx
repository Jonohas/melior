import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

// The height is a `cva` variant because it has to vary: table-density callers need
// `h-8` where the default form register is `h-9`. Everything that does NOT vary stays
// in the base string. (Either way a caller's `className="h-12"` still wins — `cn`'s
// `tailwind-merge` pass resolves height conflicts by last-wins, and does not care
// which argument the losing class came from.)
const inputVariants = cva(
    [
        'file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground',
        'border-input w-full min-w-0 rounded-md border bg-secondary px-3 py-2 text-base shadow-(--input-shadow)',
        'transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent',
        'file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50',
        'md:text-sm',
        // `color-scheme` so native affordances (autofill overlay, scrollbars) follow the theme,
        // as `date-field.tsx` does for pickers.
        '[color-scheme:light] dark:[color-scheme:dark]',
        'focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]',
        'aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive',
    ].join(' '),
    {
        variants: {
            size: {
                default: 'h-9',
                sm: 'h-8',
            },
        },
        defaultVariants: {
            size: 'default',
        },
    },
);

function Input({
    className,
    type,
    size = 'default',
    ...props
}: Omit<ComponentProps<'input'>, 'size'> & VariantProps<typeof inputVariants>) {
    return (
        <input
            type={type}
            data-slot="input"
            data-size={size}
            className={cn(
                inputVariants({ size }),
                // `py-2` leaves an 18px content box inside `h-9` (14px inside `h-8`),
                // but the file-picker button is `file:h-7` (28px) and the input is
                // `overflow: clip`, so the button loses height off each end. File
                // inputs drop the vertical padding at every size and let the full
                // content box centre the 28px button instead.
                '[&[type=file]]:py-0',
                className,
            )}
            {...props}
        />
    );
}

export { Input, inputVariants };
