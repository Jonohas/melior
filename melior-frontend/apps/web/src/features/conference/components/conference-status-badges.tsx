import { Badge, Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import {
    type ConferenceStatus,
    type DeviceTrust,
    type Path,
    roomMediaPath,
    type SessionId,
    summarizeEncryption,
} from '@/features/conference/domain';
import { encryptionLabel, roomPathLabel } from './media-path-labels';

type ConferenceStatusBadgesProps = {
    sessionId: SessionId | null;
    status: ConferenceStatus;
    reconnectAttempt: number;
    /** The other participants, by session id. */
    participants: Record<SessionId, string>;
    /** The path each remote participant is shown over; absent means forwarded. */
    shownPaths: Record<SessionId, Path>;
    /** How far each participant's device is trusted for end-to-end encryption. */
    trust: Record<SessionId, DeviceTrust>;
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

const ENCRYPTION_VARIANT = { secure: 'success', waiting: 'secondary', attention: 'warning' } as const;

/** Whether the call's media is encrypted end to end, and whose devices need a second look. */
const EncryptionBadge = ({
    sessionId,
    participants,
    trust,
}: Pick<ConferenceStatusBadgesProps, 'sessionId' | 'participants' | 'trust'>) => {
    const summary = summarizeEncryption(participants, trust, sessionId ? trust[sessionId] : undefined);
    if (summary.kind === 'alone') return null;
    const { text, hint } = encryptionLabel(summary);
    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <Badge variant={ENCRYPTION_VARIANT[summary.kind]}>{text}</Badge>
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
    trust,
}: ConferenceStatusBadgesProps) => (
    <>
        <StatusBadge status={status} reconnectAttempt={reconnectAttempt} />
        <MediaPathBadge participants={participants} shownPaths={shownPaths} />
        <EncryptionBadge sessionId={sessionId} participants={participants} trust={trust} />
        {sessionId && (
            <span className="font-mono text-xs text-muted-foreground" title="Session id">
                session {sessionId.slice(0, 8)}…
            </span>
        )}
    </>
);
