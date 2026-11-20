import { useNavigate } from '@tanstack/react-router';
import { MonitorUp, PhoneCall, TriangleAlert, Users } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { StatePage, TopBar } from '@/components';
import { Alert, AlertDescription, Badge, Button, Card, CardContent } from '@/components/ui';
import { ChatPanel } from '@/features/chat/components';
import {
    type Channel,
    type CommunityView,
    decideVoiceOpen,
    firstChannel,
    hasPermission,
    isInCall,
    mediaSummary,
    occupantsOf,
    type VoiceTarget,
} from '@/features/communities/domain';
import {
    useCommunityFeed,
    useVoiceSession,
    type VoiceCall,
    type VoiceSessionValue,
} from '@/features/communities/hooks';
import {
    ConferenceStatusBadges,
    LocalPreview,
    RemoteVideoGrid,
    RoomDock,
    StoppedNotice,
} from '@/features/conference/components';
import { CallDeviceSettings } from './call-device-settings';
import { ChannelIcon } from './channel-tree';
import { MemberListButton } from './member-list';

type VoiceChannelViewProps = {
    view: CommunityView;
    channel: Channel;
    /** Force all media through the TURN relay (diagnostics). */
    relay: boolean;
};

const CallNotices = ({ call }: { call: VoiceCall }) => {
    const { conference } = call;
    const error = conference.error ?? call.authNotice ?? conference.screenError;
    return (
        <>
            {error && (
                <Alert variant="destructive" role="alert">
                    <TriangleAlert />
                    <AlertDescription>{error}</AlertDescription>
                </Alert>
            )}
            {conference.screenSharing && (
                <Alert role="status">
                    <MonitorUp />
                    <AlertDescription className="flex items-center justify-between gap-2">
                        You are sharing your screen with everyone in the channel.
                        <Button type="button" size="sm" variant="outline" onClick={conference.stopScreenShare}>
                            Stop sharing
                        </Button>
                    </AlertDescription>
                </Alert>
            )}
        </>
    );
};

const CallStage = ({ call }: { call: VoiceCall }) => {
    const { conference } = call;
    const alone = Object.keys(conference.participants).length === 0;
    return (
        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
            <CallNotices call={call} />
            <RemoteVideoGrid
                streams={conference.remoteStreams}
                directStreams={conference.directStreams}
                directScreens={conference.directScreens}
                shownPaths={conference.shownPaths}
                usernames={conference.participants}
                screens={conference.screenStreams}
                voices={conference.voices}
                local={
                    <LocalPreview
                        stream={conference.localStream}
                        username={conference.username}
                        microphoneMuted={conference.microphoneMuted}
                        cameraOff={conference.cameraOff}
                        speaking={conference.speaking}
                    />
                }
            />
            {alone && (
                <Card className="mx-auto w-full max-w-xl border-dashed">
                    <CardContent className="flex flex-col items-center gap-2 py-6 text-center">
                        <Users className="size-5 text-muted-foreground" aria-hidden="true" />
                        <p className="font-medium">You are the only one here</p>
                        <p className="text-sm text-muted-foreground">Others join from the channel list.</p>
                    </CardContent>
                </Card>
            )}
        </div>
    );
};

/** Opening a voice channel joins its call, leaving another one first; a later leave on this page does not rejoin. */
const useJoinOnOpen = (voice: VoiceSessionValue, target: VoiceTarget) => {
    const sessionRef = useRef(voice.session);
    sessionRef.current = voice.session;
    const { join } = voice;
    useEffect(() => {
        const decision = decideVoiceOpen(sessionRef.current, target);
        if (decision === 'join' || decision === 'switch') join(target);
    }, [join, target]);
};

/** Leaving goes to another channel of the server, preferably a text one, never back into this call. */
const useGoElsewhere = (view: CommunityView, channel: Channel): (() => void) => {
    const navigate = useNavigate();
    return () => {
        const next = firstChannel(view, channel.id);
        if (!next) return void navigate({ to: '/' });
        void navigate({ to: '/c/$communityId/$channelId', params: { communityId: view.id, channelId: next.id } });
    };
};

type CallViewProps = { channel: Channel; voice: VoiceSessionValue; onLeave: () => void };

/** The call you are in: the grid, its chat beside it, and the controls. */
const CallView = ({ channel, voice, onLeave }: CallViewProps) => {
    const { roster } = useCommunityFeed();
    const { call } = voice;
    const { conference, chat, chatOpen, setChatOpen } = call;
    const [devicesOpen, setDevicesOpen] = useState(false);
    const summary = mediaSummary(channel, occupantsOf(roster, channel.id).length);
    const connected = conference.status === 'connected';
    const mayWrite = hasPermission(channel.myPermissions, 'send_messages');

    return (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <TopBar
                title={
                    <>
                        <ChannelIcon channel={channel} className="text-muted-foreground" />
                        {channel.name}
                    </>
                }
                subtitle={channel.topic || undefined}
            >
                <ConferenceStatusBadges
                    sessionId={conference.sessionId}
                    status={conference.status}
                    reconnectAttempt={conference.reconnectAttempt}
                    participants={conference.participants}
                    shownPaths={conference.shownPaths}
                />
                {summary && <Badge variant="outline">{summary}</Badge>}
                <MemberListButton />
            </TopBar>
            <div className="flex min-h-0 flex-1 flex-col lg:flex-row">
                <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                    <CallStage call={call} />
                    <RoomDock
                        chatOpen={chatOpen}
                        onToggleChat={() => setChatOpen(!chatOpen)}
                        unread={chat.state.unread}
                        screenShareSupported={conference.screenShareSupported}
                        screenSharing={conference.screenSharing}
                        connected={connected}
                        onOpenDevices={() => setDevicesOpen(true)}
                        microphoneMuted={conference.microphoneMuted}
                        onToggleMicrophone={() => conference.setMicrophoneMuted(!conference.microphoneMuted)}
                        cameraOff={conference.cameraOff}
                        onToggleCamera={() => conference.setCameraOff(!conference.cameraOff)}
                        onToggleScreenShare={() =>
                            conference.screenSharing ? conference.stopScreenShare() : void conference.startScreenShare()
                        }
                        onLeave={onLeave}
                    />
                </div>
                {chatOpen && (
                    <ChatPanel
                        state={chat.state}
                        ownName={conference.username}
                        canSend={connected && mayWrite}
                        pausedReason={mayWrite ? undefined : 'You do not have permission to send messages here.'}
                        onSend={(draft) => chat.submit(draft, conference.sendChat)}
                        onRetry={(clientId) => chat.retry(clientId, conference.sendChat)}
                        onLoadEarlier={chat.loadEarlier}
                        onReloadHistory={chat.reloadHistory}
                        onDismissNotice={chat.dismissNotice}
                        onClose={() => setChatOpen(false)}
                    />
                )}
            </div>
            <CallDeviceSettings
                open={devicesOpen}
                onOpenChange={setDevicesOpen}
                activeDevices={conference.activeDevices}
                changeInput={conference.changeInput}
                speakerId={voice.speakerId}
                onSpeakerChange={voice.setSpeakerId}
            />
        </div>
    );
};

/**
 * A voice channel. Opening it joins its call (the call itself lives above the routes, so it goes on elsewhere);
 * the page shows the call while you are in it, why the server ended it, or a way to join again.
 */
export const VoiceChannelView = ({ view, channel, relay }: VoiceChannelViewProps) => {
    const voice = useVoiceSession();
    const target = useMemo(
        () => ({
            communityId: view.id,
            channelId: channel.id,
            relay,
            communityName: view.name,
            channelName: channel.name,
        }),
        [view.id, channel.id, relay, view.name, channel.name],
    );
    useJoinOnOpen(voice, target);
    const goElsewhere = useGoElsewhere(view, channel);
    const { ended } = voice.session;

    if (isInCall(voice.session, channel.id)) {
        const leave = () => {
            voice.leave();
            goElsewhere();
        };
        return <CallView channel={channel} voice={voice} onLeave={leave} />;
    }
    const rejoin = (
        <Button type="button" variant="outline" onClick={() => voice.join(target)}>
            <PhoneCall /> Join the call
        </Button>
    );
    if (ended?.target.channelId === channel.id) {
        const back = () => {
            voice.dismiss();
            goElsewhere();
        };
        const actions = (
            <>
                <Button type="button" onClick={back}>
                    Back to {view.name}
                </Button>
                {rejoin}
            </>
        );
        return <StoppedNotice reason={ended.reason} actions={actions} />;
    }
    return (
        <StatePage icon={PhoneCall} title={`You are not in ${channel.name}`} actions={rejoin}>
            Join to see and hear everyone in this channel.
        </StatePage>
    );
};
