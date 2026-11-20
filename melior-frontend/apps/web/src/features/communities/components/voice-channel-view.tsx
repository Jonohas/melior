import { useNavigate } from '@tanstack/react-router';
import { MonitorUp, TriangleAlert, Users } from 'lucide-react';
import { useState } from 'react';
import { TopBar } from '@/components';
import { Alert, AlertDescription, Badge, Button, Card, CardContent } from '@/components/ui';
import { ChatPanel } from '@/features/chat/components';
import {
    type Channel,
    type CommunityView,
    firstChannel,
    hasPermission,
    mediaSummary,
    occupantsOf,
} from '@/features/communities/domain';
import { useCommunityFeed, useVoiceCall, type VoiceCall } from '@/features/communities/hooks';
import { loadDevicePreferences } from '@/features/conference/api';
import {
    ConferenceStatusBadges,
    LocalPreview,
    RemoteVideoGrid,
    RoomDock,
    StoppedNotice,
} from '@/features/conference/components';
import { CallDeviceSettings } from './call-device-settings';
import { ChannelIcon } from './channel-tree';

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

const CallStage = ({ call, speakerId }: { call: VoiceCall; speakerId: string }) => {
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
                speakerId={speakerId}
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

/** A voice channel: the call, its chat beside it, and the controls. */
export const VoiceChannelView = ({ view, channel, relay }: VoiceChannelViewProps) => {
    const navigate = useNavigate();
    const { roster } = useCommunityFeed();
    const call = useVoiceCall(channel.id, relay);
    const { conference, chat, chatOpen, setChatOpen } = call;
    const [devicesOpen, setDevicesOpen] = useState(false);
    const [speakerId, setSpeakerId] = useState(() => loadDevicePreferences().audiooutput ?? '');
    const summary = mediaSummary(channel, occupantsOf(roster, channel.id).length);
    const connected = conference.status === 'connected';
    const mayWrite = hasPermission(channel.myPermissions, 'send_messages');
    // Leaving goes to another channel of the server, preferably a text one, never back into this call.
    const leave = () => {
        const next = firstChannel(view, channel.id);
        if (!next) return void navigate({ to: '/' });
        void navigate({ to: '/c/$communityId/$channelId', params: { communityId: view.id, channelId: next.id } });
    };

    if (conference.stopReason) {
        const back = (
            <Button type="button" onClick={leave}>
                Back to {view.name}
            </Button>
        );
        return <StoppedNotice reason={conference.stopReason} actions={back} />;
    }

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
            </TopBar>
            <div className="flex min-h-0 flex-1 flex-col lg:flex-row">
                <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                    <CallStage call={call} speakerId={speakerId} />
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
                        onLeave={leave}
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
                speakerId={speakerId}
                onSpeakerChange={setSpeakerId}
            />
        </div>
    );
};
