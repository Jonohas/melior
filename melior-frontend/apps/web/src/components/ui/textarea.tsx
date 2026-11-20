import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

// The min-height is a `cva` variant because it has to vary: compact callers need
// `min-h-12` where the default form register is `min-h-16`. Everything that does NOT
// vary stays in the base string. (Either way a caller's `className="min-h-40"` still
// wins — `cn`'s `tailwind-merge` pass resolves the conflict by last-wins, and does not
// care which argument the losing class came from.)
const textareaVariants = cva(
    [
        'border-input placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50',
        'aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive',
        'flex field-sizing-content w-full rounded-md border bg-secondary px-3 py-2 text-base shadow-(--input-shadow)',
        'transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed',
        'disabled:opacity-50 md:text-sm',
    ].join(' '),
    {
        variants: {
            size: {
                default: 'min-h-16',
                sm: 'min-h-12',
            },
        },
        defaultVariants: {
            size: 'default',
        },
    },
);

function Textarea({
    className,
    size = 'default',
    ...props
}: ComponentProps<'textarea'> & VariantProps<typeof textareaVariants>) {
    return (
        <textarea
            data-slot="textarea"
            data-size={size}
            className={cn(textareaVariants({ size }), className)}
            {...props}
        />
    );
}

export { Textarea, textareaVariants };
