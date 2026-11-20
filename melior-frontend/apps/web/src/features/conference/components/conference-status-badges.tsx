import { Badge, Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import { type ConferenceStatus, type Path, roomMediaPath, type SessionId } from '@/features/conference/domain';
import { roomPathLabel } from './media-path-labels';

type ConferenceStatusBadgesProps = {
    sessionId: SessionId | null;
    status: ConferenceStatus;
    reconnectAttempt: number;
    /** The other participants, by session id. */
    participants: Record<SessionId, string>;
    /** The path each remote participant is shown over; absent means forwarded. */
    shownPaths: Record<SessionId, Path>;
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

/** Whether the room's media goes between browsers or through the server; nothing while alone. */
const MediaPathBadge = ({
    participants,
    shownPaths,
}: Pick<ConferenceStatusBadgesProps, 'participants' | 'shownPaths'>) => {
    const path = roomMediaPath(Object.keys(participants), shownPaths);
    if (path.kind === 'none') return null;
    const { text, hint } = roomPathLabel(path);
    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <Badge variant={path.kind === 'peer_to_peer' ? 'success' : 'outline'}>{text}</Badge>
            </TooltipTrigger>
            <TooltipContent>{hint}</TooltipContent>
        </Tooltip>
    );
};

/** Where the room connection stands, how its media travels, and which session it is. */
export const ConferenceStatusBadges = ({
    sessionId,
    status,
    reconnectAttempt,
    participants,
    shownPaths,
}: ConferenceStatusBadgesProps) => (
    <>
        <StatusBadge status={status} reconnectAttempt={reconnectAttempt} />
        <MediaPathBadge participants={participants} shownPaths={shownPaths} />
        {sessionId && (
            <span className="font-mono text-xs text-muted-foreground" title="Session id">
                session {sessionId.slice(0, 8)}…
            </span>
        )}
    </>
);
