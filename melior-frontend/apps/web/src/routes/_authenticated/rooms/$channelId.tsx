import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { Lock, MonitorUp, Plus, TriangleAlert, Users } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { TopBar } from '@/components';
import { Alert, AlertDescription, Badge, Button, Card, CardContent } from '@/components/ui';
import { ChatPanel } from '@/features/chat/components';
import { useChat } from '@/features/chat/hooks';
import { canChooseSpeaker, loadDevicePreferences, saveDevicePreference } from '@/features/conference/api';
import {
    ConferenceStatusBadges,
    DeviceSettingsDialog,
    LocalPreview,
    RemoteVideoGrid,
    RoomDock,
    StoppedNotice,
} from '@/features/conference/components';
import { isChannelId, parseChannelId, parseForceRelay } from '@/features/conference/domain';
import { type ChatEvent, useConference, useMediaDevices } from '@/features/conference/hooks';
import { probeRoomAccess } from '@/features/rooms/api';
import { RoomAccessDialog } from '@/features/rooms/components';
import { useRoomQuery, useSaveRoomMutation } from '@/features/rooms/hooks';
import { recheckSession } from '@/lib/auth';
import { queryKeys } from '@/lib/query-keys';

type RoomSearch = {
    relay?: boolean;
};

export const Route = createFileRoute('/_authenticated/rooms/$channelId')({
    // `?relay=1` (or `true`) forces all media through the TURN relay, to verify that path works.
    validateSearch: (raw: Record<string, unknown>): RoomSearch => (parseForceRelay(raw.relay) ? { relay: true } : {}),
    beforeLoad: ({ params }) => {
        if (!isChannelId(params.channelId)) throw redirect({ to: '/' });
    },
    component: RoomPage,
});

function RoomPage() {
    const params = Route.useParams();
    const { relay } = Route.useSearch();
    const router = useRouter();
    const queryClient = useQueryClient();
    const [authNotice, setAuthNotice] = useState<string | null>(null);
    const [chatOpen, setChatOpen] = useState(true);
    const [accessOpen, setAccessOpen] = useState(false);
    const [devicesOpen, setDevicesOpen] = useState(false);
    const [deviceError, setDeviceError] = useState<string | null>(null);
    const [speakerId, setSpeakerId] = useState(() => loadDevicePreferences().audiooutput ?? '');
    // The chat needs the connection's status and the connection needs the chat's inbox: this ref joins them.
    const chatInbox = useRef<(event: ChatEvent) => void>(() => undefined);
    const channelId = parseChannelId(params.channelId);
    const {
        sessionId,
        username,
        participants,
        localStream,
        remoteStreams,
        directStreams,
        directScreens,
        shownPaths,
        screenStreams,
        screenShareSupported,
        screenSharing,
        screenError,
        startScreenShare,
        stopScreenShare,
        activeDevices,
        changeInput,
        microphoneMuted,
        cameraOff,
        setMicrophoneMuted,
        setCameraOff,
        status,
        reconnectAttempt,
        stopReason,
        error,
        sendChat,
    } = useConference(channelId, {
        forceRelay: relay === true,
        // After a lost connection the conference asks whether the session is still good: a 401 runs the
        // login gate (redirect, or a refusal message), a 403 shows the no-access page.
        checkSession: async () => {
            const result = await recheckSession(queryClient, setAuthNotice);
            if (result === 'forbidden') void router.invalidate();
            return result;
        },
        checkRoomAccess: () => probeRoomAccess(channelId),
        onChat: (event) => chatInbox.current(event),
    });
    const chat = useChat(channelId, { connected: status === 'connected', username, open: chatOpen });
    useEffect(() => {
        chatInbox.current = chat.receive;
    }, [chat.receive]);

    const devices = useMediaDevices(devicesOpen);
    const room = useRoomQuery(channelId);
    const saveRoom = useSaveRoomMutation(channelId);

    // Joining registers an unregistered room, so refresh its access settings once connected.
    useEffect(() => {
        if (status === 'connected') {
            void queryClient.invalidateQueries({ queryKey: queryKeys.rooms.detail(channelId) });
        }
    }, [status, channelId, queryClient]);

    if (stopReason) {
        return (
            <>
                <TopBar room={channelId} />
                <StoppedNotice reason={stopReason} />
            </>
        );
    }

    const switchInput = (kind: 'audioinput' | 'videoinput', deviceId: string) => {
        setDeviceError(null);
        changeInput(kind, deviceId).catch((error) => {
            const what = kind === 'audioinput' ? 'microphone' : 'camera';
            setDeviceError(`Could not switch the ${what}: ${error instanceof Error ? error.message : String(error)}`);
        });
    };

    const canManageAccess = room.data?.isOwner === true;
    const alone = Object.keys(participants).length === 0;

    return (
        <>
            <TopBar room={channelId}>
                <ConferenceStatusBadges sessionId={sessionId} status={status} reconnectAttempt={reconnectAttempt} />
                {room.data && (
                    <Badge variant="outline">
                        {room.data.restricted ? (
                            <>
                                <Lock /> Invite only
                            </>
                        ) : (
                            'Open to all members'
                        )}
                    </Badge>
                )}
                {room.data && <span className="text-xs text-muted-foreground">Owner: {room.data.owner}</span>}
                <Badge variant="secondary">
                    <Users /> {Object.keys(participants).length + 1} in room
                </Badge>
            </TopBar>
            <div className="flex min-h-0 flex-1 flex-col lg:flex-row">
                <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
                        {(error || authNotice || screenError) && (
                            <Alert variant="destructive" role="alert">
                                <TriangleAlert />
                                <AlertDescription>{error ?? authNotice ?? screenError}</AlertDescription>
                            </Alert>
                        )}
                        {screenSharing && (
                            <Alert role="status">
                                <MonitorUp />
                                <AlertDescription className="flex items-center justify-between gap-2">
                                    You are sharing your screen with everyone in the room.
                                    <Button type="button" size="sm" variant="outline" onClick={stopScreenShare}>
                                        Stop sharing
                                    </Button>
                                </AlertDescription>
                            </Alert>
                        )}
                        <RemoteVideoGrid
                            streams={remoteStreams}
                            directStreams={directStreams}
                            directScreens={directScreens}
                            shownPaths={shownPaths}
                            usernames={participants}
                            screens={screenStreams}
                            speakerId={speakerId}
                            local={
                                <LocalPreview
                                    stream={localStream}
                                    username={username}
                                    microphoneMuted={microphoneMuted}
                                    cameraOff={cameraOff}
                                />
                            }
                        />
                        {alone && (
                            <Card className="mx-auto w-full max-w-xl border-dashed">
                                <CardContent className="flex flex-col items-center gap-2 py-6 text-center">
                                    <Users className="size-5 text-muted-foreground" aria-hidden="true" />
                                    <p className="font-medium">You are the only one here</p>
                                    <p className="text-sm text-muted-foreground">
                                        Invite people so they can join this room.
                                    </p>
                                    {canManageAccess && (
                                        <Button type="button" onClick={() => setAccessOpen(true)}>
                                            <Plus /> Invite people
                                        </Button>
                                    )}
                                </CardContent>
                            </Card>
                        )}
                    </div>
                    <RoomDock
                        canManageAccess={canManageAccess}
                        onOpenAccess={() => setAccessOpen(true)}
                        chatOpen={chatOpen}
                        onToggleChat={() => setChatOpen((open) => !open)}
                        unread={chat.state.unread}
                        screenShareSupported={screenShareSupported}
                        screenSharing={screenSharing}
                        connected={status === 'connected'}
                        onOpenDevices={() => setDevicesOpen(true)}
                        microphoneMuted={microphoneMuted}
                        onToggleMicrophone={() => setMicrophoneMuted(!microphoneMuted)}
                        cameraOff={cameraOff}
                        onToggleCamera={() => setCameraOff(!cameraOff)}
                        onToggleScreenShare={() => (screenSharing ? stopScreenShare() : void startScreenShare())}
                    />
                </div>
                {chatOpen && (
                    <ChatPanel
                        state={chat.state}
                        ownName={username}
                        canSend={status === 'connected'}
                        onSend={(draft) => chat.submit(draft, sendChat)}
                        onRetry={(clientId) => chat.retry(clientId, sendChat)}
                        onLoadEarlier={chat.loadEarlier}
                        onReloadHistory={chat.reloadHistory}
                        onDismissNotice={chat.dismissNotice}
                        onClose={() => setChatOpen(false)}
                    />
                )}
            </div>
            <DeviceSettingsDialog
                open={devicesOpen}
                onOpenChange={(open) => {
                    setDevicesOpen(open);
                    if (!open) setDeviceError(null);
                }}
                microphones={devices.audioinput}
                cameras={devices.videoinput}
                speakers={devices.audiooutput}
                activeMicrophone={activeDevices.audioinput}
                activeCamera={activeDevices.videoinput}
                activeSpeaker={speakerId}
                canChooseSpeaker={canChooseSpeaker()}
                onChangeMicrophone={(deviceId) => switchInput('audioinput', deviceId)}
                onChangeCamera={(deviceId) => switchInput('videoinput', deviceId)}
                onChangeSpeaker={(deviceId) => {
                    setSpeakerId(deviceId);
                    saveDevicePreference('audiooutput', deviceId);
                }}
                error={deviceError}
            />
            {room.data?.isOwner && (
                <RoomAccessDialog
                    room={room.data}
                    open={accessOpen}
                    onOpenChange={setAccessOpen}
                    saving={saveRoom.isPending}
                    saveError={saveRoom.error?.message ?? null}
                    onSave={(settings) => saveRoom.mutate(settings)}
                />
            )}
        </>
    );
}
