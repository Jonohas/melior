import { useNavigate } from '@tanstack/react-router';
import { type FormEvent, useId, useState } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    Input,
    Label,
    NativeSelect,
} from '@/components/ui';
import { type ChannelKind, type CommunityView, checkName } from '@/features/communities/domain';
import { useCreateChannel } from '@/features/communities/hooks';
import { cn } from '@/lib/utils';
import { ChannelIcon } from './channel-tree';
import { FormError } from './form-error';

type CreateChannelDialogProps = {
    view: CommunityView;
    /** The category to put it in; null for none. */
    categoryId: string | null;
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

const KIND_TEXT: Record<ChannelKind, { label: string; hint: string }> = {
    text: { label: 'Text', hint: 'Messages, kept in the history.' },
    voice: { label: 'Voice', hint: 'Video, voice and screen sharing, with a chat beside the call.' },
};

const KindChoice = ({ value, onChange }: { value: ChannelKind; onChange: (kind: ChannelKind) => void }) => (
    <fieldset className="flex flex-col gap-2">
        <legend className="mb-1 text-sm font-medium">Channel type</legend>
        {(['text', 'voice'] as const).map((kind) => (
            <label
                key={kind}
                className={cn(
                    'flex cursor-pointer items-center gap-3 rounded-md border border-border p-3',
                    value === kind && 'border-primary bg-accent',
                )}
            >
                <input
                    type="radio"
                    name="channel-kind"
                    className="sr-only"
                    checked={value === kind}
                    onChange={() => onChange(kind)}
                />
                <ChannelIcon channel={{ kind }} />
                <span className="flex flex-col">
                    <span className="text-sm font-medium">{KIND_TEXT[kind].label}</span>
                    <span className="text-xs text-muted-foreground">{KIND_TEXT[kind].hint}</span>
                </span>
            </label>
        ))}
    </fieldset>
);

/** Creates a channel and opens it. */
export const CreateChannelDialog = ({ view, categoryId, open, onOpenChange }: CreateChannelDialogProps) => {
    const nameId = useId();
    const categoryFieldId = useId();
    const navigate = useNavigate();
    const create = useCreateChannel(view.id);
    const [kind, setKind] = useState<ChannelKind>('text');
    const [name, setName] = useState('');
    const [category, setCategory] = useState(categoryId ?? '');
    const [invalid, setInvalid] = useState<string | null>(null);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const checked = checkName(name);
        setInvalid(checked.ok ? null : checked.error);
        if (!checked.ok) return;
        create.mutate(
            { name: checked.value, kind, categoryId: category === '' ? null : category },
            {
                onSuccess: (channel) => {
                    onOpenChange(false);
                    void navigate({
                        to: '/c/$communityId/$channelId',
                        params: { communityId: view.id, channelId: channel.id },
                    });
                },
            },
        );
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Create channel</DialogTitle>
                    <DialogDescription>A new channel takes its category's permissions.</DialogDescription>
                </DialogHeader>
                <form onSubmit={handleSubmit} className="flex flex-col gap-4">
                    <KindChoice value={kind} onChange={setKind} />
                    <div className="flex flex-col gap-1.5">
                        <Label htmlFor={nameId}>Channel name</Label>
                        <Input id={nameId} value={name} onChange={(event) => setName(event.target.value)} />
                    </div>
                    <div className="flex flex-col gap-1.5">
                        <Label htmlFor={categoryFieldId}>Category</Label>
                        <NativeSelect
                            id={categoryFieldId}
                            value={category}
                            onChange={(event) => setCategory(event.target.value)}
                        >
                            <option value="">No category</option>
                            {view.categories.map((entry) => (
                                <option key={entry.id} value={entry.id}>
                                    {entry.name}
                                </option>
                            ))}
                        </NativeSelect>
                    </div>
                    <FormError message={invalid ?? create.error?.message} />
                    <DialogFooter>
                        <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                            Cancel
                        </Button>
                        <Button type="submit" disabled={create.isPending}>
                            Create channel
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
};
