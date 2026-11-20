import { Badge } from '@/components/ui';
import type { ConferenceStatus, SessionId } from '@/features/conference/domain';

type ConferenceStatusBadgesProps = {
    sessionId: SessionId | null;
    status: ConferenceStatus;
    reconnectAttempt: number;
};

const STATUS_LABEL: Record<Exclude<ConferenceStatus, 'connected' | 'reconnecting'>, string> = {
    starting: 'Starting',
    connecting: 'Connecting',
    negotiating: 'Negotiating',
    stopped: 'Stopped',
    error: 'Error',
};

const StatusBadge = ({
    status,
    reconnectAttempt,
}: Pick<ConferenceStatusBadgesProps, 'status' | 'reconnectAttempt'>) => {
    if (status === 'connected') return <Badge variant="success">Connected</Badge>;
    if (status === 'reconnecting') return <Badge variant="warning">Reconnecting… (attempt {reconnectAttempt})</Badge>;
    return <Badge variant={status === 'error' ? 'destructive' : 'secondary'}>{STATUS_LABEL[status]}</Badge>;
};

/** Where the room connection stands, and which session it is. */
export const ConferenceStatusBadges = ({ sessionId, status, reconnectAttempt }: ConferenceStatusBadgesProps) => (
    <>
        <StatusBadge status={status} reconnectAttempt={reconnectAttempt} />
        {sessionId && (
            <span className="font-mono text-xs text-muted-foreground" title="Session id">
                session {sessionId.slice(0, 8)}…
            </span>
        )}
    </>
);
