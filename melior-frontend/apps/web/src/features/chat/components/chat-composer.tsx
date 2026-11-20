import { Send, TriangleAlert } from 'lucide-react';
import { type FormEvent, type KeyboardEvent, useState } from 'react';
import { Alert, AlertDescription, Button, Textarea } from '@/components/ui';
import { checkDraft, type DraftCheck, MAX_CHAT_CHARS } from '@/features/chat/domain';
import { cn } from '@/lib/utils';

type ChatComposerProps = {
    /** False while there is no connection to the room: the draft is kept, sending is paused. */
    canSend: boolean;
    onSend: (draft: string) => DraftCheck;
};

const COUNTER_FROM = Math.floor(MAX_CHAT_CHARS * 0.8);

const REFUSALS: Record<Extract<DraftCheck, { ok: false }>['reason'], string> = {
    empty: 'Write a message first.',
    too_long: `A message is at most ${MAX_CHAT_CHARS} characters.`,
    invalid_character: 'A message cannot contain control characters.',
};

export const ChatComposer = ({ canSend, onSend }: ChatComposerProps) => {
    const [draft, setDraft] = useState('');
    const [refusal, setRefusal] = useState<string | null>(null);
    const length = Array.from(draft).length;
    const tooLong = length > MAX_CHAT_CHARS;
    const sendable = canSend && !tooLong && checkDraft(draft).ok;

    const submit = () => {
        if (!canSend) return;
        const result = onSend(draft);
        setRefusal(result.ok ? null : REFUSALS[result.reason]);
        if (result.ok) setDraft('');
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

    return (
        <form onSubmit={handleSubmit} className="flex flex-col gap-2 border-t border-border p-3">
            {!canSend && (
                <Alert role="status">
                    <TriangleAlert />
                    <AlertDescription>
                        Sending is paused while the connection is down. Your draft is kept.
                    </AlertDescription>
                </Alert>
            )}
            <div className="flex items-end gap-2">
                <Textarea
                    aria-label="Message"
                    rows={2}
                    value={draft}
                    disabled={!canSend}
                    placeholder="Message the room"
                    onChange={(event) => {
                        setDraft(event.target.value);
                        setRefusal(null);
                    }}
                    onKeyDown={handleKeyDown}
                    aria-invalid={tooLong || refusal !== null}
                    className="min-h-12 flex-1 resize-none"
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
            {refusal && (
                <p className="text-xs text-destructive" role="alert">
                    {refusal}
                </p>
            )}
        </form>
    );
};
