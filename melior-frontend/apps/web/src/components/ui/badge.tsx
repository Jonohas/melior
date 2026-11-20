import { cva, type VariantProps } from 'class-variance-authority';
import { Slot } from 'radix-ui';
import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

const badgeVariants = cva(
    'inline-flex items-center justify-center rounded-full border px-2 py-0.5 text-xs font-medium w-fit whitespace-nowrap shrink-0 [&>svg]:size-3 gap-1 [&>svg]:pointer-events-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive transition-[color,box-shadow] overflow-hidden',
    {
        variants: {
            variant: {
                default: 'border-transparent bg-primary text-primary-foreground [a&]:hover:bg-primary/90',
                secondary: 'border-transparent bg-secondary text-secondary-foreground [a&]:hover:bg-secondary/90',
                destructive:
                    // Keeps `dark:bg-destructive/60`, unlike the Button variant. A badge is a label, not a
                    // control, so Button's "a destructive action may not be quieter than a safe one" rule
                    // does not bind it. Here the alpha separates this red from `default` in dark mode and
                    // raises white-text contrast against the fill. Status mappers must not pair `default`
                    // with `destructive`: give a healthy state `success` or `information` instead.
                    'border-transparent bg-destructive text-white [a&]:hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60',
                outline: 'text-foreground [a&]:hover:bg-accent [a&]:hover:text-accent-foreground',
                // A quiet descriptive tag (tactic, source) that should not compete with the card's content.
                subtle: 'rounded-md border-transparent bg-muted font-normal text-muted-foreground [a&]:hover:bg-muted/80',
                warning: 'border-transparent bg-accent text-accent-foreground [a&]:hover:bg-accent/90',
                information: 'border-transparent bg-primary text-primary-foreground [a&]:hover:bg-primary/90',
                success: 'border-transparent bg-success text-primary-foreground [a&]:hover:bg-success/90',
            },
        },
        defaultVariants: {
            variant: 'default',
        },
    },
);

function Badge({
    className,
    variant,
    asChild = false,
    ...props
}: ComponentProps<'span'> & VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
    const Comp = asChild ? Slot.Root : 'span';

    return (
        <Comp
            data-slot="badge"
            data-variant={variant}
            className={cn(badgeVariants({ variant }), className)}
            {...props}
        />
    );
}

export { Badge, badgeVariants };
