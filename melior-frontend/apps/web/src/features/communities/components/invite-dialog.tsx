import { Check, Copy } from 'lucide-react';
import { useId, useState } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Input,
    Label,
    NativeSelect,
} from '@/components/ui';
import { type CommunityView, INVITE_MAX_AGES, INVITE_MAX_USES, inviteLink } from '@/features/communities/domain';
import { useCreateInvite } from '@/features/communities/hooks';
import { FormError } from './form-error';

type InviteDialogProps = {
    view: CommunityView;
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

const DAY_SECONDS = 86400;

/** The finished link, with a copy button. */
const InviteLinkField = ({ link }: { link: string }) => {
    const [copied, setCopied] = useState(false);
    const copy = () => {
        navigator.clipboard
            .writeText(link)
            .then(() => setCopied(true))
            .catch(() => setCopied(false));
    };
    return (
        <div className="flex gap-2">
            <Input readOnly value={link} aria-label="Invite link" onFocus={(event) => event.target.select()} />
            <Button type="button" variant="outline" onClick={copy}>
                {copied ? <Check /> : <Copy />} {copied ? 'Copied' : 'Copy'}
            </Button>
        </div>
    );
};

/** Creates an invite link: how long it is valid and how often it can be used. */
export const InviteDialog = ({ view, open, onOpenChange }: InviteDialogProps) => {
    const ageId = useId();
    const usesId = useId();
    const create = useCreateInvite(view.id);
    const [maxAge, setMaxAge] = useState(String(DAY_SECONDS));
    const [maxUses, setMaxUses] = useState('');
    const invite = create.data;

    const generate = () =>
        create.mutate({
            maxAgeSeconds: maxAge === '' ? null : Number(maxAge),
            maxUses: maxUses === '' ? null : Number(maxUses),
        });

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Invite people to {view.name}</DialogTitle>
                    <DialogDescription>Anyone with a Melior account and the link can join.</DialogDescription>
                </DialogHeader>
                <div className="grid gap-3 sm:grid-cols-2">
                    <div className="flex flex-col gap-1.5">
                        <Label htmlFor={ageId}>Expire after</Label>
                        <NativeSelect id={ageId} value={maxAge} onChange={(event) => setMaxAge(event.target.value)}>
                            {INVITE_MAX_AGES.map((age) => (
                                <option key={age.label} value={age.seconds === null ? '' : String(age.seconds)}>
                                    {age.label}
                                </option>
                            ))}
                        </NativeSelect>
                    </div>
                    <div className="flex flex-col gap-1.5">
                        <Label htmlFor={usesId}>Max number of uses</Label>
                        <NativeSelect id={usesId} value={maxUses} onChange={(event) => setMaxUses(event.target.value)}>
                            {INVITE_MAX_USES.map((uses) => (
                                <option key={uses ?? 'none'} value={uses === null ? '' : String(uses)}>
                                    {uses === null ? 'No limit' : `${uses} ${uses === 1 ? 'use' : 'uses'}`}
                                </option>
                            ))}
                        </NativeSelect>
                    </div>
                </div>
                <Button type="button" onClick={generate} disabled={create.isPending}>
                    {invite ? 'Generate a new link' : 'Generate link'}
                </Button>
                {invite && <InviteLinkField link={inviteLink(window.location.origin, invite.code)} />}
                <FormError message={create.error?.message} />
            </DialogContent>
        </Dialog>
    );
};
