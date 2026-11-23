import { LogOut, MessageSquare, Mic, MicOff, MonitorOff, MonitorUp, Settings2, Video, VideoOff } from 'lucide-react';
import { Badge, Button } from '@/components/ui';

type RoomDockProps = {
    chatOpen: boolean;
    onToggleChat: () => void;
    /** Chat messages that arrived while the panel was closed. */
    unread: number;
    /** False in a browser that cannot capture a screen: the button is left out. */
    screenShareSupported: boolean;
    screenSharing: boolean;
    /** Whether the room connection is up; a screen can only be shared then. */
    connected: boolean;
    onToggleScreenShare: () => void;
    onOpenDevices: () => void;
    microphoneMuted: boolean;
    onToggleMicrophone: () => void;
    cameraOff: boolean;
    onToggleCamera: () => void;
    /** Leaves the call; the caller decides where to go. */
    onLeave: () => void;
};

/**
 * A control's text: beside the icon while the dock is wide enough for all six, read out only while it is narrower
 * (a phone, or a desktop with the chat and member list open), where the labels would wrap into rows.
 */
const DockLabel = ({ children }: { children: string }) => <span className="@max-3xl:sr-only">{children}</span>;

/** The controls at the bottom of a room. */
export const RoomDock = ({
    chatOpen,
    onToggleChat,
    unread,
    screenShareSupported,
    screenSharing,
    connected,
    onToggleScreenShare,
    onOpenDevices,
    microphoneMuted,
    onToggleMicrophone,
    cameraOff,
    onToggleCamera,
    onLeave,
}: RoomDockProps) => (
    <div className="@container flex shrink-0 flex-wrap items-center justify-center gap-2 border-t border-border bg-card px-2 py-2 sm:px-4 sm:py-3">
        <Button
            type="button"
            variant={microphoneMuted ? 'destructive' : 'outline'}
            aria-pressed={microphoneMuted}
            onClick={onToggleMicrophone}
        >
            {microphoneMuted ? <MicOff /> : <Mic />}
            <DockLabel>{microphoneMuted ? 'Unmute' : 'Mute'}</DockLabel>
        </Button>
        <Button
            type="button"
            variant={cameraOff ? 'destructive' : 'outline'}
            aria-pressed={cameraOff}
            onClick={onToggleCamera}
        >
            {cameraOff ? <VideoOff /> : <Video />}
            <DockLabel>{cameraOff ? 'Camera on' : 'Camera off'}</DockLabel>
        </Button>
        <Button type="button" variant="outline" onClick={onOpenDevices}>
            <Settings2 /> <DockLabel>Audio and video</DockLabel>
        </Button>
        {screenShareSupported && (
            <Button
                type="button"
                variant={screenSharing ? 'secondary' : 'outline'}
                aria-pressed={screenSharing}
                disabled={!connected && !screenSharing}
                onClick={onToggleScreenShare}
            >
                {screenSharing ? <MonitorOff /> : <MonitorUp />}
                <DockLabel>{screenSharing ? 'Stop sharing' : 'Share screen'}</DockLabel>
            </Button>
        )}
        <Button
            type="button"
            variant={chatOpen ? 'secondary' : 'outline'}
            aria-pressed={chatOpen}
            className="relative"
            onClick={onToggleChat}
        >
            <MessageSquare /> <DockLabel>Chat</DockLabel>
            {unread > 0 && !chatOpen && (
                <Badge className="absolute -top-2 -right-2 px-1.5" aria-label={`${unread} new`}>
                    {unread}
                </Badge>
            )}
        </Button>
        <Button type="button" variant="destructive" onClick={onLeave}>
            <LogOut /> <DockLabel>Leave</DockLabel>
        </Button>
    </div>
);
