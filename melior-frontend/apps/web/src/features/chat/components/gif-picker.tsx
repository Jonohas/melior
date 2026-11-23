import { ImagePlay, Search } from 'lucide-react';
import { useState } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
    Input,
    Skeleton,
} from '@/components/ui';
import type { GifAttachment } from '@/features/chat/domain';
import { useGifSearch } from '@/features/chat/hooks';

type GifPickerProps = {
    appKey: string;
    disabled: boolean;
    /** Sends the GIF as a message of its own. Returns whether it was accepted. */
    onPick: (gif: GifAttachment) => boolean;
};

const SKELETONS = ['a', 'b', 'c', 'd', 'e', 'f'];

/** The search box, the grid and its states; mounted only while the picker is open, so nothing is asked before. */
const GifSearchPanel = ({ appKey, onPick }: { appKey: string; onPick: (gif: GifAttachment) => void }) => {
    const [input, setInput] = useState('');
    const search = useGifSearch(appKey, input);

    return (
        <div className="flex min-h-0 flex-col gap-3">
            <div className="relative">
                <Search
                    className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
                    aria-hidden="true"
                />
                <Input
                    type="search"
                    aria-label="Search GIFs"
                    placeholder="Search KLIPY"
                    value={input}
                    onChange={(event) => setInput(event.target.value)}
                    className="pl-8"
                />
            </div>
            <div className="max-h-[55dvh] min-h-40 overflow-y-auto" aria-busy={search.status === 'loading'}>
                {search.status === 'loading' && (
                    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3" role="status" aria-label="Loading GIFs">
                        {SKELETONS.map((key) => (
                            <Skeleton key={key} className="aspect-square w-full" />
                        ))}
                    </div>
                )}
                {search.status === 'error' && (
                    <div className="flex flex-col items-center gap-2 p-6 text-center text-sm">
                        <p role="alert">{search.error ?? 'Could not load GIFs.'}</p>
                        <Button type="button" variant="outline" size="sm" onClick={search.retry}>
                            Try again
                        </Button>
                    </div>
                )}
                {search.status === 'ready' && search.results.length === 0 && (
                    <p className="p-6 text-center text-sm text-muted-foreground">No GIFs found.</p>
                )}
                {search.status === 'ready' && search.results.length > 0 && (
                    <>
                        <ul className="grid list-none grid-cols-2 gap-2 sm:grid-cols-3" aria-label="GIFs">
                            {search.results.map((result) => (
                                <li key={result.id}>
                                    <button
                                        type="button"
                                        aria-label={result.title ? `Send GIF: ${result.title}` : 'Send this GIF'}
                                        className="block w-full cursor-pointer overflow-hidden rounded-md bg-muted outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                                        style={{ aspectRatio: `${result.preview.width} / ${result.preview.height}` }}
                                        onClick={() => onPick(result.gif)}
                                    >
                                        {/* biome-ignore lint/performance/noImgElement: a Next.js rule; this app has no image component to use instead */}
                                        <img
                                            src={result.preview.url}
                                            alt=""
                                            loading="lazy"
                                            width={result.preview.width}
                                            height={result.preview.height}
                                            className="size-full object-cover"
                                        />
                                    </button>
                                </li>
                            ))}
                        </ul>
                        {search.error && (
                            <p className="mt-2 text-center text-xs text-destructive" role="alert">
                                {search.error}
                            </p>
                        )}
                        {search.hasNext && (
                            <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                className="mx-auto mt-3 flex"
                                disabled={search.loadingMore}
                                onClick={search.loadMore}
                            >
                                {search.loadingMore ? 'Loading…' : 'Load more'}
                            </Button>
                        )}
                    </>
                )}
            </div>
            <p className="text-right text-xs text-muted-foreground">Powered by KLIPY</p>
        </div>
    );
};

/** The GIF button and its picker. A picked GIF is sent at once as its own message; the draft stays as it is. */
export const GifPicker = ({ appKey, disabled, onPick }: GifPickerProps) => {
    const [open, setOpen] = useState(false);

    const pick = (gif: GifAttachment) => {
        if (onPick(gif)) setOpen(false);
    };

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger asChild>
                <Button type="button" variant="ghost" size="icon" aria-label="Send a GIF" disabled={disabled}>
                    <ImagePlay />
                </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-xl">
                <DialogHeader>
                    <DialogTitle>Send a GIF</DialogTitle>
                    <DialogDescription>Pick one to send it to the channel right away.</DialogDescription>
                </DialogHeader>
                <GifSearchPanel appKey={appKey} onPick={pick} />
            </DialogContent>
        </Dialog>
    );
};
