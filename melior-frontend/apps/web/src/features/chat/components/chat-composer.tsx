import { ImagePlus, LoaderCircle, RotateCcw, Send, TriangleAlert, X } from 'lucide-react';
import { type ClipboardEvent, type FormEvent, type KeyboardEvent, useRef, useState } from 'react';
import { Alert, AlertDescription, Button, Textarea } from '@/components/ui';
import {
    checkDraft,
    type DraftCheck,
    type GifAttachment,
    IMAGE_TYPES,
    MAX_CHAT_CHARS,
    MAX_IMAGES_PER_MESSAGE,
    type OutgoingAttachment,
} from '@/features/chat/domain';
import { type ImageDraft, useGifAppKey, useImageAttachments } from '@/features/chat/hooks';
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
    onSend: (draft: string, attachments: OutgoingAttachment[]) => DraftCheck;
};

const CONNECTION_DOWN = 'Sending is paused while the connection is down. Your draft is kept.';

const COUNTER_FROM = Math.floor(MAX_CHAT_CHARS * 0.8);

const REFUSALS: Record<Extract<DraftCheck, { ok: false }>['reason'], string> = {
    empty: 'Write a message first.',
    too_long: `A message is at most ${MAX_CHAT_CHARS} characters.`,
    invalid_character: 'A message cannot contain control characters.',
    too_many_images: `A message carries at most ${MAX_IMAGES_PER_MESSAGE} images.`,
    gif_not_alone: 'A GIF is sent on its own.',
    duplicate_image: 'An image is attached twice.',
};

const IMAGE_NOT_READY: Record<Exclude<ImageDraft['status'], 'ready'>, string> = {
    uploading: 'Wait for the images to finish uploading.',
    failed: 'An image was not uploaded. Try it again or remove it.',
};

/** The images in a paste, in order. */
const pastedImages = (event: ClipboardEvent<HTMLTextAreaElement>): File[] =>
    Array.from(event.clipboardData.items)
        .filter((item) => item.kind === 'file' && item.type.startsWith('image/'))
        .map((item) => item.getAsFile())
        .filter((file): file is File => file !== null);

const ImageTile = ({
    image,
    position,
    onRetry,
    onRemove,
}: {
    image: ImageDraft;
    position: number;
    onRetry: () => void;
    onRemove: () => void;
}) => {
    const label = image.name || `Image ${position}`;
    return (
        <li
            className={cn(
                'relative size-16 shrink-0 rounded-md border border-border bg-muted/40',
                image.status === 'failed' && 'border-destructive',
            )}
        >
            {/* biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead */}
            <img src={image.previewUrl} alt={label} className="size-full rounded-md object-cover" />
            {image.status === 'uploading' && (
                <span
                    className="absolute inset-0 flex items-center justify-center rounded-md bg-background/60"
                    role="status"
                    aria-label={`Uploading ${label}`}
                >
                    <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
                </span>
            )}
            {image.status === 'failed' && (
                <span className="absolute inset-0 flex items-center justify-center rounded-md bg-background/70">
                    <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label={`Upload ${label} again`}
                        className="text-destructive"
                        onClick={onRetry}
                    >
                        <RotateCcw />
                    </Button>
                </span>
            )}
            <Button
                type="button"
                variant="secondary"
                size="icon-sm"
                aria-label={`Remove ${label}`}
                className="absolute -top-2 -right-2 size-7 rounded-full border border-border shadow-sm"
                onClick={onRemove}
            >
                <X />
            </Button>
        </li>
    );
};

const ImageTiles = ({
    images,
    onRetry,
    onRemove,
}: {
    images: ImageDraft[];
    onRetry: (key: string) => void;
    onRemove: (key: string) => void;
}) => {
    const failed = images.filter((image) => image.status === 'failed');
    return (
        <div className="flex flex-col gap-1">
            <ul aria-label="Images to send" className="flex list-none flex-wrap gap-3 pt-2 pr-2">
                {images.map((image, index) => (
                    <ImageTile
                        key={image.key}
                        image={image}
                        position={index + 1}
                        onRetry={() => onRetry(image.key)}
                        onRemove={() => onRemove(image.key)}
                    />
                ))}
            </ul>
            {failed.map((image) => (
                <p key={image.key} className="text-xs text-destructive" role="alert">
                    {image.name || 'Image'}: {image.error ?? 'The image was not uploaded.'}
                </p>
            ))}
        </div>
    );
};

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
    const images = useImageAttachments(channelId);
    const gifAppKey = useGifAppKey();
    const length = Array.from(draft).length;
    const tooLong = length > MAX_CHAT_CHARS;
    const ready = images.images.flatMap((image) => (image.attachment ? [image.attachment] : []));
    const notReady = images.images.find((image) => image.status !== 'ready');
    const sendable = canSend && !tooLong && notReady === undefined && checkDraft(draft, ready).ok;

    const submit = () => {
        if (!canSend) return;
        if (notReady && notReady.status !== 'ready') {
            setRefusal(IMAGE_NOT_READY[notReady.status]);
            return;
        }
        const result = onSend(draft, ready);
        setRefusal(result.ok ? null : REFUSALS[result.reason]);
        if (!result.ok) return;
        setDraft('');
        images.clear();
    };

    const sendGif = (gif: GifAttachment): boolean => {
        if (!canSend) return false;
        const result = onSend('', [gif]);
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
        const files = pastedImages(event);
        if (files.length === 0) return;
        event.preventDefault();
        setRefusal(null);
        images.pick(files);
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
            {images.images.length > 0 && (
                <ImageTiles images={images.images} onRetry={images.retry} onRemove={images.remove} />
            )}
            <div className="flex items-end gap-1">
                <input
                    ref={fileInput}
                    type="file"
                    accept={IMAGE_TYPES.join(',')}
                    multiple
                    className="hidden"
                    tabIndex={-1}
                    aria-hidden="true"
                    onChange={(event) => {
                        const files = Array.from(event.target.files ?? []);
                        // Cleared so that picking the same files again still fires a change.
                        event.target.value = '';
                        if (files.length === 0) return;
                        setRefusal(null);
                        images.pick(files);
                    }}
                />
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label="Attach images"
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
