import { ImagePlus, LoaderCircle, RotateCcw, Send, TriangleAlert, X } from 'lucide-react';
import { type ClipboardEvent, type FormEvent, type KeyboardEvent, useRef, useState } from 'react';
import { Alert, AlertDescription, Button, Textarea } from '@/components/ui';
import {
    checkDraft,
    type DraftCheck,
    type GifAttachment,
    IMAGE_TYPES,
    MAX_CHAT_CHARS,
    type OutgoingAttachment,
} from '@/features/chat/domain';
import { type ImageDraft, useGifAppKey, useImageAttachment } from '@/features/chat/hooks';
import { cn } from '@/lib/utils';
import { GifPicker } from './gif-picker';

type ChatComposerProps = {
    /** The channel images are uploaded into. */
    channelId: string;
    /** False while there is no connection to the room: the draft is kept, sending is paused. */
    canSend: boolean;
    /** What to say while sending is not possible; the default is about a lost connection. */
    pausedReason?: string;
    placeholder?: string;
    onSend: (draft: string, attachment: OutgoingAttachment | null) => DraftCheck;
};

const CONNECTION_DOWN = 'Sending is paused while the connection is down. Your draft is kept.';

const COUNTER_FROM = Math.floor(MAX_CHAT_CHARS * 0.8);

const REFUSALS: Record<Extract<DraftCheck, { ok: false }>['reason'], string> = {
    empty: 'Write a message first.',
    too_long: `A message is at most ${MAX_CHAT_CHARS} characters.`,
    invalid_character: 'A message cannot contain control characters.',
};

const IMAGE_NOT_READY: Record<Exclude<ImageDraft['status'], 'ready'>, string> = {
    uploading: 'Wait for the image to finish uploading.',
    failed: 'The image was not uploaded. Try again or remove it.',
};

/** The first image in a paste, if there is one. */
const pastedImage = (event: ClipboardEvent<HTMLTextAreaElement>): File | null => {
    for (const item of Array.from(event.clipboardData.items)) {
        if (item.kind === 'file' && item.type.startsWith('image/')) return item.getAsFile();
    }
    return null;
};

const ImageChip = ({ image, onRetry, onRemove }: { image: ImageDraft; onRetry: () => void; onRemove: () => void }) => (
    <div
        className={cn(
            'flex max-w-full items-center gap-2 self-start rounded-md border border-border bg-muted/40 p-1.5 pr-1',
            image.status === 'failed' && 'border-destructive',
        )}
    >
        <div className="relative size-12 shrink-0 overflow-hidden rounded">
            {/* biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead */}
            <img src={image.previewUrl} alt="Chosen image" className="size-full object-cover" />
            {image.status === 'uploading' && (
                <span className="absolute inset-0 flex items-center justify-center bg-background/60">
                    <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
                </span>
            )}
        </div>
        <div className="min-w-0 flex-1 text-xs">
            <p className="truncate font-medium">{image.name || 'Image'}</p>
            {image.status === 'uploading' && (
                <p className="text-muted-foreground" role="status">
                    Uploading…
                </p>
            )}
            {image.status === 'ready' && <p className="text-muted-foreground">Ready to send</p>}
            {image.status === 'failed' && (
                <p className="text-destructive" role="alert">
                    {image.error ?? 'The image was not uploaded.'}
                </p>
            )}
        </div>
        {image.status === 'failed' && (
            <Button type="button" variant="ghost" size="icon" aria-label="Upload the image again" onClick={onRetry}>
                <RotateCcw />
            </Button>
        )}
        <Button type="button" variant="ghost" size="icon" aria-label="Remove the image" onClick={onRemove}>
            <X />
        </Button>
    </div>
);

export const ChatComposer = ({
    channelId,
    canSend,
    pausedReason = CONNECTION_DOWN,
    placeholder = 'Message the channel',
    onSend,
}: ChatComposerProps) => {
    const [draft, setDraft] = useState('');
    const [refusal, setRefusal] = useState<string | null>(null);
    const fileInput = useRef<HTMLInputElement>(null);
    const images = useImageAttachment(channelId);
    const gifAppKey = useGifAppKey();
    const { image } = images;
    const length = Array.from(draft).length;
    const tooLong = length > MAX_CHAT_CHARS;
    const imageReady = image?.status === 'ready' ? image.attachment : null;
    const imageBlocking = image !== null && imageReady === null;
    const sendable = canSend && !tooLong && !imageBlocking && checkDraft(draft, imageReady !== null).ok;

    const submit = () => {
        if (!canSend) return;
        if (image && image.status !== 'ready') {
            setRefusal(IMAGE_NOT_READY[image.status]);
            return;
        }
        const result = onSend(draft, imageReady);
        setRefusal(result.ok ? null : REFUSALS[result.reason]);
        if (!result.ok) return;
        setDraft('');
        images.clear();
    };

    const sendGif = (gif: GifAttachment): boolean => {
        if (!canSend) return false;
        const result = onSend('', gif);
        setRefusal(result.ok ? null : REFUSALS[result.reason]);
        return result.ok;
    };

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        submit();
    };

    // Enter sends, Shift+Enter starts a new line.
    const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
        if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return;
        event.preventDefault();
        submit();
    };

    const handlePaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
        const file = pastedImage(event);
        if (!file) return;
        event.preventDefault();
        setRefusal(null);
        images.pick(file);
    };

    const notice = refusal ?? images.refusal;

    return (
        <form onSubmit={handleSubmit} className="flex flex-col gap-2 border-t border-border p-3">
            {!canSend && (
                <Alert role="status">
                    <TriangleAlert />
                    <AlertDescription>{pausedReason}</AlertDescription>
                </Alert>
            )}
            {image && <ImageChip image={image} onRetry={images.retry} onRemove={images.clear} />}
            <div className="flex items-end gap-1">
                <input
                    ref={fileInput}
                    type="file"
                    accept={IMAGE_TYPES.join(',')}
                    className="hidden"
                    tabIndex={-1}
                    aria-hidden="true"
                    onChange={(event) => {
                        const file = event.target.files?.[0];
                        // Cleared so that picking the same file again still fires a change.
                        event.target.value = '';
                        if (!file) return;
                        setRefusal(null);
                        images.pick(file);
                    }}
                />
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label="Attach image"
                    disabled={!canSend}
                    onClick={() => fileInput.current?.click()}
                >
                    <ImagePlus />
                </Button>
                {gifAppKey !== null && <GifPicker appKey={gifAppKey} disabled={!canSend} onPick={sendGif} />}
                <Textarea
                    aria-label="Message"
                    rows={2}
                    value={draft}
                    disabled={!canSend}
                    placeholder={placeholder}
                    onChange={(event) => {
                        setDraft(event.target.value);
                        setRefusal(null);
                    }}
                    onKeyDown={handleKeyDown}
                    onPaste={handlePaste}
                    aria-invalid={tooLong || refusal !== null}
                    className="min-h-12 min-w-0 flex-1 resize-none"
                />
                <Button type="submit" size="icon" aria-label="Send" disabled={!sendable}>
                    <Send />
                </Button>
            </div>
            {length >= COUNTER_FROM && (
                <p className={cn('text-xs', tooLong ? 'text-destructive' : 'text-muted-foreground')}>
                    {length} / {MAX_CHAT_CHARS}
                </p>
            )}
            {notice && (
                <p className="text-xs text-destructive" role="alert">
                    {notice}
                </p>
            )}
        </form>
    );
};
