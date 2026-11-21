import { Trash2 } from 'lucide-react';
import { Button, Skeleton } from '@/components/ui';
import { type CommunityView, describeExpiry, describeUses, type Invite } from '@/features/communities/domain';
import { useDeleteInvite, useInvitesQuery } from '@/features/communities/hooks';
import { FormError } from './form-error';

const InviteRow = ({ invite, onRevoke }: { invite: Invite; onRevoke: () => void }) => (
    <li className="flex items-center gap-3 py-2 text-sm">
        <code className="font-mono">{invite.code}</code>
        <span className="text-muted-foreground">by {invite.createdBy}</span>
        <span className="flex-1 text-right text-xs text-muted-foreground">
            {describeUses(invite)} · {describeExpiry(invite.expiresAtMs, Date.now())}
        </span>
        <Button type="button" variant="ghost" size="icon-sm" aria-label={`Revoke ${invite.code}`} onClick={onRevoke}>
            <Trash2 className="text-destructive" />
        </Button>
    </li>
);

/** Every invite of the server, with who made it and how much it was used. */
export const InvitesTab = ({ view }: { view: CommunityView }) => {
    const invites = useInvitesQuery(view.id, true);
    const revoke = useDeleteInvite(view.id);

    if (invites.isPending) return <Skeleton className="h-32" />;
    const list = invites.data ?? [];
    return (
        <div className="flex flex-col gap-2">
            <FormError message={invites.error?.message ?? revoke.error?.message} />
            {list.length === 0 && <p className="text-sm text-muted-foreground">No invites yet.</p>}
            <ul className="flex max-h-[50vh] list-none flex-col divide-y divide-border overflow-y-auto">
                {list.map((invite) => (
                    <InviteRow key={invite.code} invite={invite} onRevoke={() => revoke.mutate(invite.code)} />
                ))}
            </ul>
        </div>
    );
};
