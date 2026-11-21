import { useCallback, useEffect, useReducer, useRef } from 'react';
import { fetchHistory } from '@/features/chat/api';
import {
    type ChatMessage,
    type ChatState,
    chatReducer,
    checkDraft,
    createInitialChatState,
    type DraftCheck,
    parseChatMessage,
} from '@/features/chat/domain';
import type { ChatEvent } from '@/features/conference/hooks';

/** Sends one message to the room. Returns false when there is no open connection. */
export type ChatTransport = (text: string, clientId: string) => boolean;

type UseChatOptions = {
    /** Whether the live connection is up. History loads on every (re)connect, to catch what was missed. */
    connected: boolean;
    /** The user's own name, shown on messages still on their way. */
    username: string | null;
    /** Whether the panel is open, to count what arrives while it is not. */
    open: boolean;
};

export type UseChatReturn = {
    state: ChatState;
    /** Feeds the room's chat events (from the conference connection) into the chat. */
    receive: (event: ChatEvent) => void;
    /** A saved message from anywhere (a feed, a POST response); `clientId` names the pending one it settles. */
    receiveSaved: (message: ChatMessage, clientId: string | null) => void;
    /** The server refused a message; with a `clientId` that message is marked failed. */
    refuse: (clientId: string | null, message: string) => void;
    /** Checks and sends a draft. The result says whether it was accepted, and why not if it was not. */
    submit: (draft: string, transport: ChatTransport) => DraftCheck;
    retry: (clientId: string, transport: ChatTransport) => void;
    loadEarlier: () => void;
    reloadHistory: () => void;
    dismissNotice: () => void;
};

const NOT_CONNECTED = 'Not sent: the connection is down. Try again when it is back.';

// A client id only has to be unique among this client's own messages, so it needs no secure random source.
const newClientId = (): string => `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;

/**
 * One channel's chat: the history from the server, the live messages from the connection, and the
 * messages this client has sent and not yet seen come back. The rules are in the chat domain.
 */
export const useChat = (channelId: string, { connected, username, open }: UseChatOptions): UseChatReturn => {
    const [state, dispatch] = useReducer(chatReducer, undefined, createInitialChatState);
    const openRef = useRef(open);
    const stateRef = useRef(state);
    const oldestId = state.saved[0]?.id ?? null;
    useEffect(() => {
        openRef.current = open;
        stateRef.current = state;
    });

    // biome-ignore lint/correctness/useExhaustiveDependencies: the room changing is what resets the chat
    useEffect(() => {
        dispatch({ type: 'reset' });
    }, [channelId]);

    useEffect(() => {
        if (open) dispatch({ type: 'panel_opened' });
    }, [open]);

    const loadNewest = useCallback(
        (isCurrent: () => boolean) => {
            dispatch({ type: 'history_loading' });
            fetchHistory(channelId)
                .then((page) => {
                    if (isCurrent())
                        dispatch({ type: 'newest_loaded', messages: page.messages, hasMore: page.hasMore });
                })
                .catch((error) => {
                    console.warn('could not load the chat history', error);
                    if (isCurrent()) dispatch({ type: 'history_failed' });
                });
        },
        [channelId],
    );

    useEffect(() => {
        if (!connected) return;
        let current = true;
        loadNewest(() => current);
        return () => {
            current = false;
        };
    }, [connected, loadNewest]);

    const receiveSaved = useCallback((message: ChatMessage, clientId: string | null) => {
        // What a person sent themselves is not news to them, and neither is anything while the panel is open.
        const countUnread = clientId === null && !openRef.current;
        dispatch({ type: 'received', message, clientId, countUnread });
    }, []);

    const refuse = useCallback((clientId: string | null, message: string) => {
        dispatch({ type: 'refused', clientId, message });
    }, []);

    const receive = useCallback(
        (event: ChatEvent) => {
            if (event.channel === 'chat_error') {
                refuse(event.payload.client_id ?? null, event.payload.message);
                return;
            }
            const message = parseChatMessage(event.payload);
            if (message) receiveSaved(message, event.payload.client_id ?? null);
        },
        [receiveSaved, refuse],
    );

    const deliver = useCallback((text: string, clientId: string, transport: ChatTransport) => {
        if (!transport(text, clientId)) dispatch({ type: 'send_failed', clientId, error: NOT_CONNECTED });
    }, []);

    const submit = useCallback(
        (draft: string, transport: ChatTransport): DraftCheck => {
            const checked = checkDraft(draft);
            if (!checked.ok) return checked;
            const clientId = newClientId();
            dispatch({ type: 'send_started', clientId, username: username ?? '', text: checked.text });
            deliver(checked.text, clientId, transport);
            return checked;
        },
        [username, deliver],
    );

    const retry = useCallback(
        (clientId: string, transport: ChatTransport) => {
            const entry = stateRef.current.pending.find((pending) => pending.clientId === clientId);
            if (!entry) return;
            dispatch({ type: 'send_retried', clientId });
            deliver(entry.text, clientId, transport);
        },
        [deliver],
    );

    const loadEarlier = useCallback(() => {
        if (oldestId === null || stateRef.current.loadingEarlier) return;
        dispatch({ type: 'earlier_loading' });
        fetchHistory(channelId, oldestId)
            .then((page) => dispatch({ type: 'earlier_loaded', messages: page.messages, hasMore: page.hasMore }))
            .catch((error) => {
                console.warn('could not load earlier messages', error);
                dispatch({ type: 'earlier_failed' });
            });
    }, [channelId, oldestId]);

    const reloadHistory = useCallback(() => loadNewest(() => true), [loadNewest]);
    const dismissNotice = useCallback(() => dispatch({ type: 'notice_cleared' }), []);

    return { state, receive, receiveSaved, refuse, submit, retry, loadEarlier, reloadHistory, dismissNotice };
};
