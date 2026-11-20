import { Lock, TriangleAlert, X } from 'lucide-react';
import { Alert, AlertDescription, Button } from '@/components/ui';
import type { ChatState, DraftCheck } from '@/features/chat/domain';
import { ChatComposer } from './chat-composer';
import { ChatMessageList } from './chat-message-list';

type ChatPanelProps = {
    state: ChatState;
    ownName: string | null;
    /** Whether the room connection is up. */
    canSend: boolean;
    onSend: (draft: string) => DraftCheck;
    onRetry: (clientId: string) => void;
    onLoadEarlier: () => void;
    onReloadHistory: () => void;
    onDismissNotice: () => void;
    onClose: () => void;
};

/** The room's chat: its messages, newest at the bottom, and the box to write a new one. */
export const ChatPanel = ({
    state,
    ownName,
    canSend,
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
            className="flex max-h-[60vh] min-h-0 w-full shrink-0 flex-col border-t border-border bg-card lg:max-h-none lg:w-96 lg:border-t-0 lg:border-l"
        >
            <header className="flex items-center justify-between border-b border-border px-4 py-2">
                <h2 className="text-sm font-semibold">Room chat</h2>
                <Button type="button" variant="ghost" size="icon-sm" aria-label="Close chat" onClick={onClose}>
                    <X />
                </Button>
            </header>
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
                Saved with the room. Anyone who can join it can read this.
            </p>
            <ChatComposer canSend={canSend} onSend={onSend} />
        </aside>
    );
};
