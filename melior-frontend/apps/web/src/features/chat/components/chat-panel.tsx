import { Lock, TriangleAlert, X } from 'lucide-react';
import { Alert, AlertDescription, Button } from '@/components/ui';
import type { ChatState, DraftCheck, OutgoingAttachment } from '@/features/chat/domain';
import { cn } from '@/lib/utils';
import { ChatComposer } from './chat-composer';
import { ChatMessageList } from './chat-message-list';

type ChatPanelProps = {
    /** The channel the chat belongs to; images are uploaded into it. */
    channelId: string;
    state: ChatState;
    ownName: string | null;
    /** Whether a message can be sent now. */
    canSend: boolean;
    /** Why sending is not possible, shown while `canSend` is false. */
    pausedReason?: string;
    title?: string;
    placeholder?: string;
    /** Fills the main pane (a text channel) instead of sitting beside a call. */
    fill?: boolean;
    onSend: (draft: string, attachments: OutgoingAttachment[]) => DraftCheck;
    onRetry: (clientId: string) => void;
    onLoadEarlier: () => void;
    onReloadHistory: () => void;
    onDismissNotice: () => void;
    /** Without it the panel has no close button. */
    onClose?: () => void;
};

/** A channel's chat: its messages, newest at the bottom, and the box to write a new one. */
export const ChatPanel = ({
    channelId,
    state,
    ownName,
    canSend,
    pausedReason,
    title = 'Channel chat',
    placeholder,
    fill = false,
    onSend,
    onRetry,
    onLoadEarlier,
    onReloadHistory,
    onDismissNotice,
    onClose,
}: ChatPanelProps) => {
    return (
        <aside
            aria-label="Chat"
            className={cn(
                'flex min-h-0 w-full flex-col bg-card',
                fill
                    ? 'flex-1'
                    : 'max-h-[60vh] shrink-0 border-t border-border lg:max-h-none lg:w-96 lg:border-t-0 lg:border-l',
            )}
        >
            {!fill && (
                <header className="flex items-center justify-between border-b border-border px-4 py-2">
                    <h2 className="text-sm font-semibold">{title}</h2>
                    {onClose && (
                        <Button type="button" variant="ghost" size="icon-sm" aria-label="Close chat" onClick={onClose}>
                            <X />
                        </Button>
                    )}
                </header>
            )}
            {state.notice && (
                <Alert variant="destructive" className="m-2 w-auto">
                    <TriangleAlert />
                    <AlertDescription className="flex items-center justify-between gap-2">
                        {state.notice}
                        <Button type="button" variant="link" size="sm" className="h-auto p-0" onClick={onDismissNotice}>
                            Dismiss
                        </Button>
                    </AlertDescription>
                </Alert>
            )}
            <ChatMessageList
                state={state}
                ownName={ownName}
                onRetry={onRetry}
                onLoadEarlier={onLoadEarlier}
                onReloadHistory={onReloadHistory}
            />
            <p className="flex items-center gap-1.5 px-4 pb-2 text-xs text-muted-foreground">
                <Lock className="size-3" aria-hidden="true" />
                Saved with the channel. Anyone who can read its history can read this.
            </p>
            <ChatComposer
                channelId={channelId}
                canSend={canSend}
                pausedReason={pausedReason}
                placeholder={placeholder}
                onSend={onSend}
            />
        </aside>
    );
};
