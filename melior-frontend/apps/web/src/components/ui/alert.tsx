import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

/**
 * Callout for a message that sits inline with page content: a provisioning notice, a failed
 * saga, a "keep this key safe" warning.
 *
 * Colour note: the `--*-foreground` tokens are the *on-solid* pairs (white on a solid
 * destructive/success/info fill, near-black on solid warning). They are unreadable on the 10%
 * tint this component uses, so the tint carries `text-foreground` / `text-muted-foreground` and
 * only the icon takes the semantic hue — which stays legible in both light and dark.
 */
const alertVariants = cva('flex w-full items-start gap-3 rounded-md border p-3 text-left', {
    variants: {
        variant: {
            default: 'border-border bg-muted/50 [&>svg]:text-muted-foreground',
            info: 'border-primary/40 bg-primary/10 [&>svg]:text-primary',
            warning: 'border-border bg-muted [&>svg]:text-foreground',
            destructive: 'border-destructive/40 bg-destructive/10 [&>svg]:text-destructive',
            success: 'border-success/40 bg-success/10 [&>svg]:text-success',
        },
    },
    defaultVariants: {
        variant: 'default',
    },
});

function Alert({ className, variant, ...props }: ComponentProps<'div'> & VariantProps<typeof alertVariants>) {
    return (
        <div
            data-slot="alert"
            role="alert"
            className={cn(alertVariants({ variant }), '[&>svg]:mt-0.5 [&>svg]:size-4 [&>svg]:shrink-0', className)}
            {...props}
        />
    );
}

/** Wraps the title + description so they stack beside the icon. */
function AlertContent({ className, ...props }: ComponentProps<'div'>) {
    return <div data-slot="alert-content" className={cn('flex-1 space-y-1', className)} {...props} />;
}

function AlertTitle({ className, ...props }: ComponentProps<'p'>) {
    return <p data-slot="alert-title" className={cn('text-sm font-medium text-foreground', className)} {...props} />;
}

function AlertDescription({ className, ...props }: ComponentProps<'div'>) {
    return <div data-slot="alert-description" className={cn('text-xs text-muted-foreground', className)} {...props} />;
}

export { Alert, AlertContent, AlertDescription, AlertTitle, alertVariants };
