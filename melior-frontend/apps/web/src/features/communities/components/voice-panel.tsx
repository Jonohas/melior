import { Link, useParams } from '@tanstack/react-router';
import { Mic, MicOff, MonitorOff, MonitorUp, PhoneOff, Signal, Video, VideoOff, X } from 'lucide-react';
import { Button } from '@/components/ui';
import { type CallTone, findChannel, type VoiceTarget, voicePanelView } from '@/features/communities/domain';
import { useCommunityQuery, useVoiceSession, type VoiceCall } from '@/features/communities/hooks';
import { cn } from '@/lib/utils';

const TONE_CLASS: Record<CallTone, string> = {
    success: 'text-success',
    pending: 'text-muted-foreground',
    warning: 'text-primary',
    danger: 'text-destructive',
};

/** The call's channel and server, linking back to the call's own page. */
const CallPlace = ({ target }: { target: VoiceTarget }) => {
    const view = useCommunityQuery(target.communityId).data;
    const channelName = (view && findChannel(view, target.channelId)?.name) ?? target.channelName;
    return (
        <Link
            to="/c/$communityId/$channelId"
            params={{ communityId: target.communityId, channelId: target.channelId }}
            search={target.relay ? { relay: true } : {}}
            className="block min-w-0 truncate text-xs text-muted-foreground hover:text-foreground hover:underline"
            title="Back to the call"
        >
            {channelName} / {view?.name ?? target.communityName}
        </Link>
    );
};

const CallControls = ({ call, onLeave }: { call: VoiceCall; onLeave: () => void }) => {
    const { conference } = call;
    const { microphoneMuted, cameraOff, screenSharing } = conference;
    const toggleScreen = () => (screenSharing ? conference.stopScreenShare() : void conference.startScreenShare());
    return (
        <div className="flex items-center gap-1">
            <Button
                type="button"
                size="icon-sm"
                variant={microphoneMuted ? 'destructive' : 'ghost'}
                aria-pressed={microphoneMuted}
                aria-label={microphoneMuted ? 'Unmute' : 'Mute'}
                title={microphoneMuted ? 'Unmute' : 'Mute'}
                onClick={() => conference.setMicrophoneMuted(!microphoneMuted)}
            >
                {microphoneMuted ? <MicOff /> : <Mic />}
            </Button>
            <Button
                type="button"
                size="icon-sm"
                variant={cameraOff ? 'destructive' : 'ghost'}
                aria-pressed={cameraOff}
                aria-label={cameraOff ? 'Camera on' : 'Camera off'}
                title={cameraOff ? 'Camera on' : 'Camera off'}
                onClick={() => conference.setCameraOff(!cameraOff)}
            >
                {cameraOff ? <VideoOff /> : <Video />}
            </Button>
            {conference.screenShareSupported && (
                <Button
                    type="button"
                    size="icon-sm"
                    variant={screenSharing ? 'secondary' : 'ghost'}
                    aria-pressed={screenSharing}
                    aria-label={screenSharing ? 'Stop sharing' : 'Share screen'}
                    title={screenSharing ? 'Stop sharing' : 'Share screen'}
                    disabled={conference.status !== 'connected' && !screenSharing}
                    onClick={toggleScreen}
                >
                    {screenSharing ? <MonitorOff /> : <MonitorUp />}
                </Button>
            )}
            <Button
                type="button"
                size="icon-sm"
                variant="ghost"
                className="ml-auto text-destructive"
                aria-label="Leave the call"
                title="Leave the call"
                onClick={onLeave}
            >
                <PhoneOff />
            </Button>
        </div>
    );
};

/**
 * The call while you look at something else, Discord's "Voice connected" box: where it is, how the connection
 * stands, the switches and the way out. After the server ended the call it says why until dismissed.
 */
export const VoicePanel = ({ className }: { className?: string }) => {
    const voice = useVoiceSession();
    const { channelId } = useParams({ strict: false });
    const { conference } = voice.call;
    const panel = voicePanelView({
        session: voice.session,
        status: conference.status,
        reconnectAttempt: conference.reconnectAttempt,
        viewingChannelId: channelId ?? null,
    });
    if (panel.kind === 'hidden') return null;

    const frame = cn('flex flex-col gap-1.5 border-t border-border bg-card p-2', className);
    if (panel.kind === 'ended') {
        return (
            <section aria-label="Voice disconnected" className={frame}>
                <div className="flex items-start gap-2">
                    <p className="flex-1 text-sm font-medium text-destructive" role="alert">
                        {panel.message}
                    </p>
                    <Button type="button" size="icon-sm" variant="ghost" aria-label="Dismiss" onClick={voice.dismiss}>
                        <X />
                    </Button>
                </div>
                <CallPlace target={panel.target} />
            </section>
        );
    }
    return (
        <section aria-label="Voice connected" className={frame}>
            <div className="min-w-0">
                <p
                    className={cn('flex items-center gap-1 text-sm font-semibold', TONE_CLASS[panel.status.tone])}
                    role="status"
                >
                    <Signal className="size-4 shrink-0" aria-hidden="true" /> {panel.status.text}
                </p>
                <CallPlace target={panel.target} />
            </div>
            <CallControls call={voice.call} onLeave={voice.leave} />
        </section>
    );
};
