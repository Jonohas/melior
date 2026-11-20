import { MessageSquare, RotateCcw } from 'lucide-react';
import { useLayoutEffect, useRef } from 'react';
import { Avatar, AvatarFallback, Button, Skeleton } from '@/components/ui';
import { buildTimeline, type ChatState, type TimelineItem } from '@/features/chat/domain';
import { cn } from '@/lib/utils';

type ChatMessageListProps = {
    state: ChatState;
    ownName: string | null;
    onRetry: (clientId: string) => void;
    onLoadEarlier: () => void;
    onReloadHistory: () => void;
};

type MessageItem = Extract<TimelineItem, { kind: 'message' }>;

const NEAR_BOTTOM_PX = 48;

const timeLabel = (sentAtMs: number): string =>
    new Date(sentAtMs).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });

const DayDivider = ({ label }: { label: string }) => (
    <li className="my-2 flex items-center gap-3 text-xs text-muted-foreground" aria-hidden="true">
        <span className="h-px flex-1 bg-border" />
        {label}
        <span className="h-px flex-1 bg-border" />
    </li>
);

const MessageRow = ({
    item,
    ownName,
    onRetry,
}: {
    item: MessageItem;
    ownName: string | null;
    onRetry: (clientId: string) => void;
}) => {
    const mine = item.username === ownName;
    return (
        <li className={cn('flex gap-3 rounded-md px-2 py-1 hover:bg-muted/50', item.showHeader && 'mt-2')}>
            <div className="w-7 shrink-0">
                {item.showHeader && (
                    <Avatar className="size-7">
                        <AvatarFallback className="bg-primary/15 text-xs text-primary">
                            {item.username.slice(0, 2).toUpperCase()}
                        </AvatarFallback>
                    </Avatar>
                )}
            </div>
            <div className="min-w-0 flex-1">
                {item.showHeader && (
                    <p className="flex items-baseline gap-2 text-sm">
                        <span className={cn('font-medium', mine && 'text-primary')}>
                            {mine ? 'You' : item.username}
                        </span>
                        {item.sentAtMs !== null && (
                            <span className="text-xs text-muted-foreground">{timeLabel(item.sentAtMs)}</span>
                        )}
                    </p>
                )}
                <p
                    className={cn(
                        'text-sm break-words whitespace-pre-wrap',
                        item.status === 'sending' && 'opacity-60',
                        item.status === 'failed' && 'text-destructive',
                    )}
                >
                    {item.text}
                </p>
                {item.status === 'sending' && <p className="text-xs text-muted-foreground">Sending…</p>}
                {item.status === 'failed' && item.clientId !== null && (
                    <p className="flex items-center gap-2 text-xs text-destructive" role="alert">
                        {item.error ?? 'Not sent.'}
                        <Button
                            type="button"
                            variant="link"
                            size="sm"
                            className="h-auto gap-1 p-0 text-xs text-destructive"
                            onClick={() => onRetry(item.clientId as string)}
                        >
                            <RotateCcw className="size-3" /> Retry
                        </Button>
                    </p>
                )}
            </div>
        </li>
    );
};

const HistorySkeleton = () => (
    <div className="flex flex-1 flex-col gap-4 p-4" aria-busy="true" role="status" aria-label="Loading messages">
        {[60, 85, 45, 70].map((width) => (
            <div key={width} className="flex gap-3">
                <Skeleton className="size-7 rounded-full" />
                <div className="flex-1 space-y-2">
                    <Skeleton className="h-3 w-1/3" />
                    <Skeleton className="h-4" style={{ width: `${width}%` }} />
                </div>
            </div>
        ))}
    </div>
);

const EmptyChat = () => (
    <div className="flex flex-1 flex-col items-center justify-center gap-1 p-6 text-center text-sm text-muted-foreground">
        <MessageSquare className="mb-1 size-5" aria-hidden="true" />
        <p className="font-medium text-foreground">No messages yet</p>
        <p>Say hello to the room.</p>
    </div>
);

export const ChatMessageList = ({ state, ownName, onRetry, onLoadEarlier, onReloadHistory }: ChatMessageListProps) => {
    const items = buildTimeline(state.saved, state.pending, new Date());
    const scroller = useRef<HTMLDivElement>(null);
    const stuckToBottom = useRef(true);
    const lastHeight = useRef(0);
    const firstKey = useRef<string | null>(null);
    const lastKey = useRef<string | null>(null);

    // Follow new messages while the reader is at the bottom; keep the view still when older ones are put above.
    useLayoutEffect(() => {
        const element = scroller.current;
        if (!element) return;
        const first = items[0]?.key ?? null;
        const last = items[items.length - 1]?.key ?? null;
        const prepended = firstKey.current !== null && first !== firstKey.current && last === lastKey.current;
        if (prepended) element.scrollTop += element.scrollHeight - lastHeight.current;
        else if (stuckToBottom.current) element.scrollTop = element.scrollHeight;
        firstKey.current = first;
        lastKey.current = last;
        lastHeight.current = element.scrollHeight;
    });

    const handleScroll = () => {
        const element = scroller.current;
        if (!element) return;
        stuckToBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < NEAR_BOTTOM_PX;
        lastHeight.current = element.scrollHeight;
    };

    if (state.history === 'error' && items.length === 0) {
        return (
            <div className="flex flex-1 flex-col items-center justify-center gap-2 p-4 text-center">
                <p role="alert">Could not load the chat history.</p>
                <Button type="button" variant="outline" size="sm" onClick={onReloadHistory}>
                    Try again
                </Button>
            </div>
        );
    }
    if ((state.history === 'idle' || state.history === 'loading') && items.length === 0) return <HistorySkeleton />;
    if (items.length === 0) return <EmptyChat />;

    return (
        <div ref={scroller} onScroll={handleScroll} className="min-h-0 flex-1 overflow-y-auto p-2">
            {state.hasMore && (
                <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="mx-auto my-2 flex"
                    disabled={state.loadingEarlier}
                    onClick={onLoadEarlier}
                >
                    {state.loadingEarlier ? 'Loading…' : 'Load earlier messages'}
                </Button>
            )}
            <ol aria-label="Messages" aria-live="polite" className="list-none">
                {items.map((item) =>
                    item.kind === 'day' ? (
                        <DayDivider key={item.key} label={item.label} />
                    ) : (
                        <MessageRow key={item.key} item={item} ownName={ownName} onRetry={onRetry} />
                    ),
                )}
            </ol>
        </div>
    );
};
