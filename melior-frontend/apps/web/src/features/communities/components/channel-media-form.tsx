import { Info } from 'lucide-react';
import { type FormEvent, useId, useState } from 'react';
import { Alert, AlertDescription, Button, Input, Label, NativeSelect } from '@/components/ui';
import {
    type Channel,
    type ChannelChange,
    checkMediaSettings,
    describeMediaWarning,
    MEDIA_MODE_LABEL,
    MEDIA_MODES,
    type MediaMode,
    USER_LIMIT_MAX,
    USER_LIMIT_MIN,
} from '@/features/communities/domain';
import { FormError } from './form-error';

type ChannelMediaFormProps = {
    channel: Channel;
    pending: boolean;
    error: string | null;
    onSave: (change: ChannelChange) => void;
};

const MODE_HINTS: Record<MediaMode, string> = {
    auto: 'Peer-to-peer while the call is small, through the server when it grows.',
    peer_to_peer: 'Always browser to browser. Lowest latency, small calls only.',
    server: 'Always through the server. Scales to large calls.',
};

/** What the server makes of the settings now, e.g. "P2P · limit 4". */
const EffectiveMedia = ({ channel }: { channel: Channel }) => {
    const limit = channel.effectiveLimit === null ? 'no limit' : `limit ${channel.effectiveLimit}`;
    return (
        <Alert role="status">
            <Info />
            <AlertDescription className="flex flex-col gap-1">
                <span>
                    In effect: {MEDIA_MODE_LABEL[channel.effectiveMode]} · {limit}
                </span>
                {channel.mediaWarning && <span>{describeMediaWarning(channel.mediaWarning)}</span>}
            </AlertDescription>
        </Alert>
    );
};

/** How a voice channel's media travels, and how many may join. */
export const ChannelMediaForm = ({ channel, pending, error, onSave }: ChannelMediaFormProps) => {
    const modeId = useId();
    const limitId = useId();
    const [mode, setMode] = useState<MediaMode>(channel.mediaMode);
    const [limit, setLimit] = useState(channel.userLimit === null ? '' : String(channel.userLimit));
    const [invalid, setInvalid] = useState<string | null>(null);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const checked = checkMediaSettings(mode, limit);
        setInvalid(checked.ok ? null : checked.error);
        if (checked.ok) onSave({ mediaMode: mode, userLimit: checked.userLimit });
    };

    return (
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <EffectiveMedia channel={channel} />
            <div className="flex flex-col gap-1.5">
                <Label htmlFor={modeId}>Media mode</Label>
                <NativeSelect
                    id={modeId}
                    value={mode}
                    onChange={(event) => setMode(MEDIA_MODES.find((m) => m === event.target.value) ?? 'auto')}
                >
                    {MEDIA_MODES.map((entry) => (
                        <option key={entry} value={entry}>
                            {MEDIA_MODE_LABEL[entry]}
                        </option>
                    ))}
                </NativeSelect>
                <p className="text-xs text-muted-foreground">{MODE_HINTS[mode]}</p>
            </div>
            <div className="flex flex-col gap-1.5">
                <Label htmlFor={limitId}>User limit</Label>
                <Input
                    id={limitId}
                    inputMode="numeric"
                    placeholder="No limit"
                    value={limit}
                    onChange={(event) => setLimit(event.target.value)}
                    aria-invalid={invalid !== null}
                />
                <p className="text-xs text-muted-foreground">
                    Empty for no limit, otherwise {USER_LIMIT_MIN} to {USER_LIMIT_MAX}.
                </p>
            </div>
            <FormError message={invalid ?? error} />
            <div className="flex justify-end">
                <Button type="submit" disabled={pending}>
                    Save media settings
                </Button>
            </div>
        </form>
    );
};
