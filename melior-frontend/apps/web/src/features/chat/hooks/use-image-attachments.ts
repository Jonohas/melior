import { useCallback, useEffect, useRef, useState } from 'react';
import { uploadImage } from '@/features/chat/api';
import {
    admitImages,
    checkImageFile,
    describeImageRefusal,
    describeLeftOut,
    type OutgoingAttachment,
} from '@/features/chat/domain';
import { UnauthorizedError } from '@/lib/auth';

/** One image chosen for the draft: uploaded as soon as it is picked, so sending only names its id. */
export type ImageDraft = {
    /** Tells the chips apart; chosen by this client. */
    key: string;
    /** A local URL of the chosen file, for the preview chip. */
    previewUrl: string;
    name: string;
    status: 'uploading' | 'ready' | 'failed';
    /** Why the upload failed, in words fit to show. */
    error: string | null;
    /** Set once the upload is done. */
    attachment: OutgoingAttachment | null;
};

export type UseImageAttachmentsReturn = {
    /** In the order they were picked, which is the order they are sent in. */
    images: ImageDraft[];
    /** Why picked files were not taken (wrong type, too large, too many), to show until the next pick. */
    refusal: string | null;
    /** Checks and uploads picked or pasted files, after the images already chosen, up to the message's limit. */
    pick: (files: readonly File[]) => void;
    retry: (key: string) => void;
    remove: (key: string) => void;
    /** Drops every chosen image, after the message was sent. */
    clear: () => void;
};

const reasonOf = (error: unknown): string => {
    if (error instanceof UnauthorizedError) return 'Your session has ended. Sign in again to send images.';
    return error instanceof Error ? error.message : 'The image was not uploaded.';
};

// Only has to be unique among this draft's images.
const newKey = (): string => `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;

/** One draft's images: the client-side checks, the uploads into the channel, their previews and failures. */
export const useImageAttachments = (channelId: string): UseImageAttachmentsReturn => {
    const [images, setImages] = useState<ImageDraft[]>([]);
    const [refusal, setRefusal] = useState<string | null>(null);
    // The files and preview URLs by key, kept outside the state so a retry can upload again and a removal can free.
    const files = useRef(new Map<string, { file: File; previewUrl: string }>());
    // An upload that settles after its image was removed (or the draft cleared) changes nothing.
    const attempts = useRef(new Map<string, number>());
    const count = useRef(0);
    useEffect(() => {
        count.current = images.length;
    });

    const update = useCallback((key: string, change: Partial<ImageDraft>) => {
        setImages((current) => current.map((image) => (image.key === key ? { ...image, ...change } : image)));
    }, []);

    const upload = useCallback(
        (key: string, file: File) => {
            const attempt = (attempts.current.get(key) ?? 0) + 1;
            attempts.current.set(key, attempt);
            const current = () => attempts.current.get(key) === attempt;
            uploadImage(channelId, file)
                .then((uploaded) => {
                    if (current())
                        update(key, { status: 'ready', error: null, attachment: { kind: 'image', id: uploaded.id } });
                })
                .catch((error: unknown) => {
                    if (current()) update(key, { status: 'failed', error: reasonOf(error) });
                });
        },
        [channelId, update],
    );

    const forget = useCallback((key: string) => {
        const entry = files.current.get(key);
        if (entry) URL.revokeObjectURL(entry.previewUrl);
        files.current.delete(key);
        attempts.current.delete(key);
    }, []);

    const pick = useCallback(
        (picked: readonly File[]) => {
            const refusals: string[] = [];
            const usable = picked.filter((file) => {
                const checked = checkImageFile(file);
                if (!checked.ok) refusals.push(`${file.name || 'A file'}: ${describeImageRefusal(checked.reason)}`);
                return checked.ok;
            });
            const { admitted, leftOut } = admitImages(count.current, usable);
            if (leftOut > 0) refusals.push(describeLeftOut(leftOut));
            setRefusal(refusals.length > 0 ? refusals.join(' ') : null);
            if (admitted.length === 0) return;
            count.current += admitted.length;
            const added = admitted.map((file): ImageDraft => {
                const key = newKey();
                const previewUrl = URL.createObjectURL(file);
                files.current.set(key, { file, previewUrl });
                return { key, previewUrl, name: file.name, status: 'uploading', error: null, attachment: null };
            });
            setImages((current) => [...current, ...added]);
            for (const image of added) {
                const entry = files.current.get(image.key);
                if (entry) upload(image.key, entry.file);
            }
        },
        [upload],
    );

    const retry = useCallback(
        (key: string) => {
            const entry = files.current.get(key);
            if (!entry) return;
            update(key, { status: 'uploading', error: null });
            upload(key, entry.file);
        },
        [update, upload],
    );

    const remove = useCallback(
        (key: string) => {
            forget(key);
            count.current = Math.max(0, count.current - 1);
            setImages((current) => current.filter((image) => image.key !== key));
            setRefusal(null);
        },
        [forget],
    );

    const clear = useCallback(() => {
        for (const key of [...files.current.keys()]) forget(key);
        count.current = 0;
        setImages([]);
        setRefusal(null);
    }, [forget]);

    // A draft belongs to its channel: switching channels (or leaving) drops the images picked for the old one.
    // biome-ignore lint/correctness/useExhaustiveDependencies: the channel changing is what drops the images
    useEffect(() => clear, [channelId, clear]);

    return { images, refusal, pick, retry, remove, clear };
};
