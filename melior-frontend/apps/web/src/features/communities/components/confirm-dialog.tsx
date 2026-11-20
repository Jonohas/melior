import type { ReactNode } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui';
import { FormError } from './form-error';

type ConfirmDialogProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    title: string;
    children: ReactNode;
    confirmLabel: string;
    pending: boolean;
    error: string | null;
    onConfirm: () => void;
};

/** Asks before something that cannot be undone: leaving, deleting, kicking. */
export const ConfirmDialog = ({
    open,
    onOpenChange,
    title,
    children,
    confirmLabel,
    pending,
    error,
    onConfirm,
}: ConfirmDialogProps) => (
    <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
            <DialogHeader>
                <DialogTitle>{title}</DialogTitle>
                <DialogDescription>{children}</DialogDescription>
            </DialogHeader>
            <FormError message={error} />
            <DialogFooter>
                <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                    Cancel
                </Button>
                <Button type="button" variant="destructive" disabled={pending} onClick={onConfirm}>
                    {confirmLabel}
                </Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
);
