import { attachmentUrl } from '@/features/chat/api';
import { ATTACHMENT_BOX_PX, fitWithin, type OutgoingAttachment } from '@/features/chat/domain';
import { cn } from '@/lib/utils';

/** The side of each image when a message carries several, so they sit together in a compact wrap. */
const GROUPED_BOX_PX = 160;

type AttachmentProps = {
    username: string;
    /** Still on its way or refused: shown dimmed. */
    pending: boolean;
};

/** A GIF, at most `ATTACHMENT_BOX_PX` on either side, loaded from KLIPY's URL as it is, as KLIPY requires. */
const GifView = ({
    gif,
    username,
    pending,
}: AttachmentProps & { gif: Extract<OutgoingAttachment, { kind: 'gif' }> }) => {
    const size = fitWithin(gif.width, gif.height);
    return (
        // biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead
        <img
            src={gif.url}
            alt={`GIF from ${username}`}
            loading="lazy"
            width={size.width}
            height={size.height}
            style={{ aspectRatio: `${gif.width} / ${gif.height}` }}
            className={cn('block h-auto max-w-full rounded-md bg-muted', pending && 'opacity-60')}
        />
    );
};

/** A saved image, bounded to `box` on either side; it opens in full in a new tab. */
const ImageView = ({ id, box, alt, pending }: { id: string; box: number; alt: string; pending: boolean }) => {
    const url = attachmentUrl(id);
    return (
        <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            style={{ maxWidth: `min(100%, ${box}px)` }}
            className="block w-fit rounded-md focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
        >
            {/* biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead */}
            <img
                src={url}
                alt={alt}
                loading="lazy"
                style={{ maxHeight: box }}
                className={cn('block h-auto max-w-full rounded-md bg-muted object-contain', pending && 'opacity-60')}
            />
        </a>
    );
};

/**
 * What a message carries, in order: a single image or GIF at most `ATTACHMENT_BOX_PX` on either side, several
 * images together in a wrap of smaller ones. Each image opens in full in a new tab.
 */
export const MessageAttachments = ({
    attachments,
    username,
    pending,
}: AttachmentProps & { attachments: readonly OutgoingAttachment[] }) => {
    const images = attachments.filter((attachment) => attachment.kind === 'image').length;
    const box = images > 1 ? GROUPED_BOX_PX : ATTACHMENT_BOX_PX;
    return (
        <ul aria-label={images > 1 ? `${images} images` : undefined} className="mt-1 flex list-none flex-wrap gap-1">
            {attachments.map((attachment, index) => (
                <li key={`${index}-${attachment.kind}`} className="max-w-full">
                    {attachment.kind === 'gif' ? (
                        <GifView gif={attachment} username={username} pending={pending} />
                    ) : (
                        <ImageView
                            id={attachment.id}
                            box={box}
                            alt={
                                images > 1
                                    ? `Image ${index + 1} of ${images} from ${username}`
                                    : `Image from ${username}`
                            }
                            pending={pending}
                        />
                    )}
                </li>
            ))}
        </ul>
    );
};
