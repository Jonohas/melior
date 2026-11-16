import { type FormEvent, useState } from 'react';
import { type ChannelId, DEFAULT_CHANNEL_ID, normalizeChannelInput } from '@/features/conference/domain';

type ChannelFormProps = {
    onSubmit: (channelId: ChannelId) => void;
};

export const ChannelForm = ({ onSubmit }: ChannelFormProps) => {
    const [value, setValue] = useState('');
    const [invalid, setInvalid] = useState(false);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const channelId = normalizeChannelInput(value);
        setInvalid(channelId === null);
        if (channelId) onSubmit(channelId);
    };

    return (
        <form onSubmit={handleSubmit} className="flex flex-col items-center gap-2">
            <label htmlFor="channel" className="text-body1">
                Channel name
            </label>
            <div className="flex gap-2">
                <input
                    id="channel"
                    name="channel"
                    type="text"
                    value={value}
                    placeholder={DEFAULT_CHANNEL_ID}
                    onChange={(event) => setValue(event.target.value)}
                    aria-invalid={invalid}
                    className="rounded border border-(--border-color) bg-(--input-background-color) px-3 py-2"
                />
                <button type="submit" className="rounded bg-(--color-primary) px-4 py-2 text-(--fg-color-primary)">
                    Join
                </button>
            </div>
            {invalid && (
                <p className="text-red-500" role="alert">
                    Use 1 to 64 letters, digits, dashes or underscores.
                </p>
            )}
        </form>
    );
};
