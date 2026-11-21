import { type FormEvent, useId, useState } from 'react';
import { Button, Input, Label, NativeSelect, Textarea } from '@/components/ui';
import {
    type Channel,
    type ChannelChange,
    type CommunityView,
    checkName,
    checkTopic,
} from '@/features/communities/domain';
import { FormError } from './form-error';

type ChannelOverviewFormProps = {
    view: CommunityView;
    channel: Channel;
    pending: boolean;
    error: string | null;
    onSave: (change: ChannelChange) => void;
    onDelete: () => void;
};

/** Name, topic and category of a channel. */
export const ChannelOverviewForm = ({ view, channel, pending, error, onSave, onDelete }: ChannelOverviewFormProps) => {
    const nameId = useId();
    const topicId = useId();
    const categoryId = useId();
    const [name, setName] = useState(channel.name);
    const [topic, setTopic] = useState(channel.topic);
    const [category, setCategory] = useState(channel.categoryId ?? '');
    const [invalid, setInvalid] = useState<string | null>(null);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const checkedName = checkName(name);
        const checkedTopic = checkTopic(topic);
        if (!checkedName.ok) {
            setInvalid(checkedName.error);
            return;
        }
        if (!checkedTopic.ok) {
            setInvalid(checkedTopic.error);
            return;
        }
        setInvalid(null);
        onSave({ name: checkedName.value, topic: checkedTopic.value, categoryId: category === '' ? null : category });
    };

    return (
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
                <Label htmlFor={nameId}>Channel name</Label>
                <Input id={nameId} value={name} onChange={(event) => setName(event.target.value)} />
            </div>
            <div className="flex flex-col gap-1.5">
                <Label htmlFor={topicId}>Topic</Label>
                <Textarea id={topicId} rows={2} value={topic} onChange={(event) => setTopic(event.target.value)} />
            </div>
            <div className="flex flex-col gap-1.5">
                <Label htmlFor={categoryId}>Category</Label>
                <NativeSelect id={categoryId} value={category} onChange={(event) => setCategory(event.target.value)}>
                    <option value="">No category</option>
                    {view.categories.map((entry) => (
                        <option key={entry.id} value={entry.id}>
                            {entry.name}
                        </option>
                    ))}
                </NativeSelect>
            </div>
            <FormError message={invalid ?? error} />
            <div className="flex justify-between gap-2">
                <Button type="button" variant="destructive" onClick={onDelete}>
                    Delete channel
                </Button>
                <Button type="submit" disabled={pending}>
                    Save changes
                </Button>
            </div>
        </form>
    );
};
