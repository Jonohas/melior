import { Mic, Video, Volume2 } from 'lucide-react';
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
import { type DeviceOption, selectedDevice } from '@/features/conference/domain';

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

/** Where you pick the microphone, camera and speaker the call uses. A change applies at once. */
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
