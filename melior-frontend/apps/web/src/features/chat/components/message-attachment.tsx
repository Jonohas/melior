import { attachmentUrl } from '@/features/chat/api';
import { ATTACHMENT_BOX_PX, fitWithin, type OutgoingAttachment } from '@/features/chat/domain';
import { cn } from '@/lib/utils';

type MessageAttachmentProps = {
    attachment: OutgoingAttachment;
    username: string;
    /** Still on its way or refused: shown dimmed. */
    pending: boolean;
};

/**
 * An image or GIF in a message, at most `ATTACHMENT_BOX_PX` on either side. An image opens in full in a new tab;
 * a GIF is loaded from KLIPY's URL as it is, as KLIPY requires.
 */
export const MessageAttachment = ({ attachment, username, pending }: MessageAttachmentProps) => {
    if (attachment.kind === 'gif') {
        const size = fitWithin(attachment.width, attachment.height);
        return (
            // biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead
            <img
                src={attachment.url}
                alt={`GIF from ${username}`}
                loading="lazy"
                width={size.width}
                height={size.height}
                style={{ aspectRatio: `${attachment.width} / ${attachment.height}` }}
                className={cn('mt-1 block h-auto max-w-full rounded-md bg-muted', pending && 'opacity-60')}
            />
        );
    }
    const url = attachmentUrl(attachment.id);
    return (
        <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            style={{ maxWidth: `min(100%, ${ATTACHMENT_BOX_PX}px)` }}
            className="mt-1 block w-fit rounded-md focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
        >
            {/* biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead */}
            <img
                src={url}
                alt={`Image from ${username}`}
                loading="lazy"
                style={{ maxHeight: ATTACHMENT_BOX_PX }}
                className={cn('block h-auto max-w-full rounded-md bg-muted object-contain', pending && 'opacity-60')}
            />
        </a>
    );
};
