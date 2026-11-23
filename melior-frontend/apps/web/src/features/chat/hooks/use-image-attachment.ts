import { useCallback, useEffect, useRef, useState } from 'react';
import { uploadImage } from '@/features/chat/api';
import { checkImageFile, describeImageRefusal, type OutgoingAttachment } from '@/features/chat/domain';
import { UnauthorizedError } from '@/lib/auth';

/** The image chosen for the draft: uploaded as soon as it is picked, so sending only names its id. */
export type ImageDraft = {
    /** A local URL of the chosen file, for the preview chip. */
    previewUrl: string;
    name: string;
    status: 'uploading' | 'ready' | 'failed';
    /** Why the upload failed, in words fit to show. */
    error: string | null;
    /** Set once the upload is done. */
    attachment: OutgoingAttachment | null;
};

export type UseImageAttachmentReturn = {
    image: ImageDraft | null;
    /** A refusal of a picked file before any upload (wrong type, too large), to show once. */
    refusal: string | null;
    /** Checks and uploads a picked or pasted file, replacing any image already chosen. */
    pick: (file: File) => void;
    retry: () => void;
    /** Drops the chosen image, for the remove button and after the message was sent. */
    clear: () => void;
};

const reasonOf = (error: unknown): string => {
    if (error instanceof UnauthorizedError) return 'Your session has ended. Sign in again to send images.';
    return error instanceof Error ? error.message : 'The image was not uploaded.';
};

/** One draft's image: the client-side check, the upload into the channel, its preview and its failure. */
export const useImageAttachment = (channelId: string): UseImageAttachmentReturn => {
    const [image, setImage] = useState<ImageDraft | null>(null);
    const [refusal, setRefusal] = useState<string | null>(null);
    const file = useRef<File | null>(null);
    // Only the newest upload may change the draft; an older one that settles late is ignored.
    const generation = useRef(0);
    const previewUrl = useRef<string | null>(null);

    /** Frees the preview of the image that is being replaced or dropped. */
    const swapPreview = useCallback((next: string | null) => {
        if (previewUrl.current !== null) URL.revokeObjectURL(previewUrl.current);
        previewUrl.current = next;
    }, []);

    const upload = useCallback(
        (chosen: File) => {
            const current = ++generation.current;
            uploadImage(channelId, chosen)
                .then((uploaded) => {
                    if (current !== generation.current) return;
                    setImage(
                        (draft) =>
                            draft && {
                                ...draft,
                                status: 'ready',
                                error: null,
                                attachment: { kind: 'image', id: uploaded.id },
                            },
                    );
                })
                .catch((error: unknown) => {
                    if (current !== generation.current) return;
                    setImage((draft) => draft && { ...draft, status: 'failed', error: reasonOf(error) });
                });
        },
        [channelId],
    );

    const clear = useCallback(() => {
        generation.current += 1;
        file.current = null;
        swapPreview(null);
        setImage(null);
        setRefusal(null);
    }, [swapPreview]);

    const pick = useCallback(
        (chosen: File) => {
            const checked = checkImageFile(chosen);
            if (!checked.ok) {
                setRefusal(describeImageRefusal(checked.reason));
                return;
            }
            file.current = chosen;
            const url = URL.createObjectURL(chosen);
            swapPreview(url);
            setRefusal(null);
            setImage({
                previewUrl: url,
                name: chosen.name,
                status: 'uploading',
                error: null,
                attachment: null,
            });
            upload(chosen);
        },
        [upload, swapPreview],
    );

    const retry = useCallback(() => {
        const chosen = file.current;
        if (!chosen) return;
        setImage((draft) => draft && { ...draft, status: 'uploading', error: null });
        upload(chosen);
    }, [upload]);

    // A draft belongs to its channel: switching channels drops the image picked for the old one.
    // biome-ignore lint/correctness/useExhaustiveDependencies: the channel changing is what drops the image
    useEffect(() => clear, [channelId, clear]);

    return { image, refusal, pick, retry, clear };
};
