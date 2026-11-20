import { type FormEvent, useState } from 'react';
import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, Label } from '@/components/ui';
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
        <Card>
            <CardHeader>
                <CardTitle>Join or create a room</CardTitle>
                <CardDescription>A room that does not exist yet is created, and you own it.</CardDescription>
            </CardHeader>
            <CardContent>
                <form onSubmit={handleSubmit} className="flex max-w-md flex-col gap-2">
                    <Label htmlFor="channel">Channel name</Label>
                    <div className="flex gap-2">
                        <Input
                            id="channel"
                            name="channel"
                            type="text"
                            value={value}
                            placeholder={DEFAULT_CHANNEL_ID}
                            onChange={(event) => setValue(event.target.value)}
                            aria-invalid={invalid}
                        />
                        <Button type="submit">Join</Button>
                    </div>
                    {invalid && (
                        <p className="text-sm text-destructive" role="alert">
                            Use 1 to 64 letters, digits, dashes or underscores.
                        </p>
                    )}
                </form>
            </CardContent>
        </Card>
    );
};
