import { Bug, Check, ChevronUp, Copy } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import { Badge, Button, Card, CardContent, CardHeader, CardTitle } from '@/components/ui';
import {
    type CallDebugSnapshot,
    type ConnectionDebug,
    type DebugLogLine,
    debugReport,
    formatBitrate,
    formatBytes,
    formatCounts,
    formatInOut,
    formatPair,
    type PeerDebug,
    timeOfDay,
} from '@/features/conference/domain';
import { type CallDebug, useCallDebug } from '@/features/conference/hooks';

/** The channel's media settings as the community view states them, for the room section and the report. */
export type ChannelMediaFacts = {
    mediaMode: string;
    effectiveMode: string;
    userLimit: number | null;
    effectiveLimit: number | null;
    mediaWarning: string | null;
};

type CallDebugPanelProps = { debug: CallDebug; channelMedia: ChannelMediaFacts };

const Row = ({ label, children }: { label: string; children: ReactNode }) => (
    <div className="grid grid-cols-[9rem_1fr] gap-2">
        <dt className="text-muted-foreground">{label}</dt>
        <dd className="break-all">{children}</dd>
    </div>
);

const Section = ({ title, children }: { title: string; children: ReactNode }) => (
    <section className="flex flex-col gap-1">
        <h3 className="font-semibold">{title}</h3>
        <dl className="flex flex-col gap-0.5">{children}</dl>
    </section>
);

const ConnectionRows = ({ connection }: { connection: ConnectionDebug }) => (
    <>
        <Row label="connection">{connection.connectionState}</Row>
        <Row label="ICE connection">{connection.iceConnectionState}</Row>
        <Row label="ICE gathering">{connection.iceGatheringState}</Row>
        <Row label="pair (local ⇄ remote)">{formatPair(connection.selectedPair)}</Row>
        <Row label="bytes">
            in {formatBytes(connection.bytesReceived)}, out {formatBytes(connection.bytesSent)}
        </Row>
        <Row label="bitrate">{formatBitrate(connection.bitrate)}</Row>
    </>
);

const RoomSection = ({ snapshot, channelMedia }: { snapshot: CallDebugSnapshot; channelMedia: ChannelMediaFacts }) => {
    const { room } = snapshot;
    return (
        <Section title="Room">
            <Row label="routes ask for">{room.requestedPath}</Row>
            <Row label="shown over">{room.shownPath}</Row>
            <Row label="SFU uplink">{room.uplink}</Row>
            <Row label="channel media">
                {channelMedia.mediaMode} (in effect {channelMedia.effectiveMode}, limit{' '}
                {channelMedia.effectiveLimit ?? 'none'})
            </Row>
            <Row label="media warning">{channelMedia.mediaWarning ?? 'none'}</Row>
            <Row label="ICE servers">
                {room.iceServerUrls === null ? 'no session yet' : room.iceServerUrls.join(', ') || 'none'}
            </Row>
            <Row label="TURN provided">{room.turnProvided ? 'yes' : 'no'}</Row>
            <Row label="relay only (?relay)">{room.forceRelay ? 'yes' : 'no'}</Row>
            <Row label="connections">{room.sessions}</Row>
        </Section>
    );
};

const SfuSection = ({ sfu }: { sfu: CallDebugSnapshot['sfu'] }) => (
    <Section title="SFU connection">
        {sfu ? (
            <>
                <ConnectionRows connection={sfu} />
                <Row label="candidates sent">{formatCounts(sfu.localCandidates)}</Row>
                <Row label="candidates received">{formatCounts(sfu.remoteCandidates)}</Row>
            </>
        ) : (
            <Row label="state">not connected</Row>
        )}
    </Section>
);

const PeerSection = ({ peer }: { peer: PeerDebug }) => (
    <Section title={`Direct link: ${peer.peer}`}>
        <Row label="route">
            <Badge variant={peer.route === 'Direct' ? 'success' : 'outline'}>{peer.route}</Badge> epoch{' '}
            {peer.epoch ?? '—'}, shown {peer.shownPath}
        </Row>
        <Row label="since ToDirect">{peer.sinceToDirect ?? '—'}</Row>
        <Row label="live after">{peer.liveAfter ?? '—'}</Row>
        {peer.link ? <ConnectionRows connection={peer.link} /> : <Row label="link">not open</Row>}
        <Row label="candidates gathered">{formatCounts(peer.localCandidates)}</Row>
        <Row label="candidates received">{formatCounts(peer.remoteCandidates)}</Row>
        <Row label="offers / answers">
            {formatInOut(peer.offers)} · {formatInOut(peer.answers)}
        </Row>
        <Row label="ICE messages">{formatInOut(peer.ice)}</Row>
    </Section>
);

const DIRECTION_MARK: Record<DebugLogLine['direction'], string> = { in: '←', out: '→', local: '·' };

const LogSection = ({ log }: { log: DebugLogLine[] }) => (
    <section className="flex flex-col gap-1">
        <h3 className="font-semibold">Events (UTC, newest last)</h3>
        <ol className="flex flex-col gap-0.5">
            {log.map((line) => (
                <li key={line.seq} className="break-all">
                    <span className="text-muted-foreground">{timeOfDay(line.time)}</span>{' '}
                    {DIRECTION_MARK[line.direction]} {line.text}
                </li>
            ))}
        </ol>
    </section>
);

const CopyReport = ({ report }: { report: () => string }) => {
    const [copied, setCopied] = useState(false);
    const copy = () => {
        navigator.clipboard
            .writeText(report())
            .then(() => setCopied(true))
            .catch(() => setCopied(false));
    };
    return (
        <Button type="button" size="sm" variant="outline" onClick={copy}>
            {copied ? <Check /> : <Copy />} {copied ? 'Copied' : 'Copy'}
        </Button>
    );
};

const PanelBody = ({ debug, channelMedia }: CallDebugPanelProps) => {
    const snapshot = useCallDebug(debug);
    if (!snapshot) return <p className="text-muted-foreground">Reading the connections…</p>;
    return (
        <div className="flex flex-col gap-3">
            <CopyReport report={() => debugReport(snapshot, { channelMedia })} />
            <RoomSection snapshot={snapshot} channelMedia={channelMedia} />
            <SfuSection sfu={snapshot.sfu} />
            {snapshot.peers.map((peer) => (
                <PeerSection key={peer.peer} peer={peer} />
            ))}
            <LogSection log={snapshot.log} />
        </div>
    );
};

/**
 * The call's debug panel (`?debug=true`), in the top right corner clear of the controls. It reads the connections
 * once a second only while it is open; folded, it polls nothing.
 */
export const CallDebugPanel = ({ debug, channelMedia }: CallDebugPanelProps) => {
    const [open, setOpen] = useState(true);
    if (!open) {
        return (
            <Button
                type="button"
                size="sm"
                variant="outline"
                className="fixed top-16 right-4 z-40"
                onClick={() => setOpen(true)}
            >
                <Bug /> Debug
            </Button>
        );
    }
    return (
        <Card className="fixed top-16 right-4 z-40 max-h-[70vh] w-[min(32rem,calc(100vw-2rem))] gap-3 overflow-y-auto py-3 font-mono text-xs">
            <CardHeader className="flex items-center justify-between px-3 pb-0">
                <CardTitle className="flex items-center gap-2 text-sm">
                    <Bug className="size-4" /> Call debug
                </CardTitle>
                <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    aria-label="Fold the debug panel"
                    onClick={() => setOpen(false)}
                >
                    <ChevronUp />
                </Button>
            </CardHeader>
            <CardContent className="px-3">
                <PanelBody debug={debug} channelMedia={channelMedia} />
            </CardContent>
        </Card>
    );
};
