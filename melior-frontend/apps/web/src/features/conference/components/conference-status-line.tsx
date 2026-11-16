import type { ChannelId, ConferenceStatus, SessionId } from '@/features/conference/domain';

type ConferenceStatusLineProps = {
    channelId: ChannelId;
    sessionId: SessionId | null;
    status: ConferenceStatus;
    reconnectAttempt: number;
    error: string | null;
};

export const ConferenceStatusLine = ({
    channelId,
    sessionId,
    status,
    reconnectAttempt,
    error,
}: ConferenceStatusLineProps) => {
    return (
        <>
            <div>
                Channel: {channelId} - Session: {sessionId ?? '-'} - Status:{' '}
                {status === 'reconnecting' ? `Reconnecting… (attempt ${reconnectAttempt})` : status}
            </div>
            {error && (
                <div className="text-red-500" role="alert">
                    {error}
                </div>
            )}
        </>
    );
};
