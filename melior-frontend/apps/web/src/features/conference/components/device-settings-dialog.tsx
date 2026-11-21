import { Gauge, Mic, MonitorUp, Video, Volume2 } from 'lucide-react';
import type { ReactNode } from 'react';
import { useId } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    Label,
    NativeSelect,
} from '@/components/ui';
import {
    CAMERA_QUALITIES,
    type CameraQuality,
    type DeviceOption,
    SCREEN_FRAME_RATES,
    type ScreenFrameRate,
    selectedDevice,
    type VideoQuality,
} from '@/features/conference/domain';

type DeviceSettingsDialogProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    microphones: DeviceOption[];
    cameras: DeviceOption[];
    speakers: DeviceOption[];
    /** The microphone and camera in use. */
    activeMicrophone: string | null;
    activeCamera: string | null;
    /** The speaker in use; empty means the browser's default. */
    activeSpeaker: string;
    /** False in browsers that cannot send sound to a chosen speaker: the choice is not offered. */
    canChooseSpeaker: boolean;
    onChangeMicrophone: (deviceId: string) => void;
    onChangeCamera: (deviceId: string) => void;
    onChangeSpeaker: (deviceId: string) => void;
    /** How sharp and smooth your video is; `best` caps nothing. */
    quality: VideoQuality;
    onChangeQuality: (quality: VideoQuality) => void;
    /** Why the last change failed, if it did. */
    error: string | null;
};

type DeviceFieldProps = {
    icon: ReactNode;
    label: string;
    options: DeviceOption[];
    value: string;
    onChange: (deviceId: string) => void;
};

const DeviceField = ({ icon, label, options, value, onChange }: DeviceFieldProps) => {
    const id = useId();
    return (
        <div className="flex flex-col gap-1.5">
            <Label htmlFor={id} className="gap-2">
                {icon}
                {label}
            </Label>
            <NativeSelect
                id={id}
                value={value}
                disabled={options.length === 0}
                onChange={(event) => onChange(event.target.value)}
            >
                {options.length === 0 && <option value="">None found</option>}
                {options.map((option) => (
                    <option key={option.deviceId} value={option.deviceId}>
                        {option.label}
                    </option>
                ))}
            </NativeSelect>
        </div>
    );
};

const CAMERA_QUALITY_LABEL: Record<CameraQuality, string> = {
    best: 'Best the camera offers',
    '1080p': 'Up to 1080p',
    '720p': 'Up to 720p',
    '480p': 'Up to 480p',
    '360p': 'Up to 360p',
};

const SCREEN_RATE_LABEL: Record<ScreenFrameRate, string> = {
    best: 'Smoothest available',
    '60': 'Up to 60 frames per second',
    '30': 'Up to 30 frames per second',
    '15': 'Up to 15 frames per second',
    '5': 'Up to 5 frames per second',
};

const toOptions = <T extends string>(values: readonly T[], labels: Record<T, string>): DeviceOption[] =>
    values.map((value) => ({ deviceId: value, label: labels[value] }));

/** Where you pick the microphone, camera and speaker the call uses, and how sharp your video is. A change applies at once. */
export const DeviceSettingsDialog = ({
    open,
    onOpenChange,
    microphones,
    cameras,
    speakers,
    activeMicrophone,
    activeCamera,
    activeSpeaker,
    canChooseSpeaker,
    onChangeMicrophone,
    onChangeCamera,
    onChangeSpeaker,
    quality,
    onChangeQuality,
    error,
}: DeviceSettingsDialogProps) => (
    <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
            <DialogHeader>
                <DialogTitle>Audio and video</DialogTitle>
                <DialogDescription>Choose what the call uses. Changes apply right away.</DialogDescription>
            </DialogHeader>
            <DeviceField
                icon={<Mic className="size-4" aria-hidden="true" />}
                label="Microphone"
                options={microphones}
                value={selectedDevice(microphones, activeMicrophone)}
                onChange={onChangeMicrophone}
            />
            <DeviceField
                icon={<Video className="size-4" aria-hidden="true" />}
                label="Camera"
                options={cameras}
                value={selectedDevice(cameras, activeCamera)}
                onChange={onChangeCamera}
            />
            {canChooseSpeaker ? (
                <DeviceField
                    icon={<Volume2 className="size-4" aria-hidden="true" />}
                    label="Speaker"
                    options={speakers}
                    value={selectedDevice(speakers, activeSpeaker)}
                    onChange={onChangeSpeaker}
                />
            ) : (
                <p className="text-sm text-muted-foreground">
                    This browser plays sound on the system's speaker. Change it in your system settings.
                </p>
            )}
            <DeviceField
                icon={<Gauge className="size-4" aria-hidden="true" />}
                label="Camera quality"
                options={toOptions(CAMERA_QUALITIES, CAMERA_QUALITY_LABEL)}
                value={quality.camera}
                onChange={(value) =>
                    onChangeQuality({ ...quality, camera: CAMERA_QUALITIES.find((q) => q === value) ?? quality.camera })
                }
            />
            <DeviceField
                icon={<MonitorUp className="size-4" aria-hidden="true" />}
                label="Screen sharing smoothness"
                options={toOptions(SCREEN_FRAME_RATES, SCREEN_RATE_LABEL)}
                value={quality.screenFrameRate}
                onChange={(value) =>
                    onChangeQuality({
                        ...quality,
                        screenFrameRate: SCREEN_FRAME_RATES.find((rate) => rate === value) ?? quality.screenFrameRate,
                    })
                }
            />
            {error && (
                <p className="text-sm text-destructive" role="alert">
                    {error}
                </p>
            )}
            <DialogFooter>
                <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                    Done
                </Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
);
