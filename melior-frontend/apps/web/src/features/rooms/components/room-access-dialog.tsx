import { X } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import {
    Avatar,
    AvatarFallback,
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    Input,
    Label,
    Switch,
} from '@/components/ui';
import { addMember, type Room, type RoomSettings, removeMember } from '@/features/rooms/domain';

type RoomAccessDialogProps = {
    room: Room;
    open: boolean;
    onOpenChange: (open: boolean) => void;
    saving: boolean;
    /** Error from the last save (for example the server rejecting a username). */
    saveError: string | null;
    onSave: (settings: RoomSettings) => void;
};

const MemberRow = ({ name, disabled, onRemove }: { name: string; disabled: boolean; onRemove: () => void }) => (
    <li className="flex items-center gap-2 rounded-md px-2 py-1 hover:bg-muted/50">
        <Avatar className="size-6">
            <AvatarFallback className="bg-primary/15 text-[10px] text-primary">
                {name.slice(0, 2).toUpperCase()}
            </AvatarFallback>
        </Avatar>
        <span className="flex-1 text-sm">{name}</span>
        <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            disabled={disabled}
            aria-label={`Remove ${name}`}
            onClick={onRemove}
        >
            <X />
        </Button>
    </li>
);

/** The owner's (or an admin's) controls: invite-only toggle and the list of invited usernames. */
export const RoomAccessDialog = ({ room, open, onOpenChange, saving, saveError, onSave }: RoomAccessDialogProps) => {
    const [input, setInput] = useState('');
    const [inputError, setInputError] = useState<string | null>(null);

    const handleAdd = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const result = addMember(room.members, input);
        if (!result.ok) {
            setInputError(result.error);
            return;
        }
        setInputError(null);
        setInput('');
        onSave({ restricted: room.restricted, members: result.members });
    };

    const error = inputError ?? saveError;

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Room access</DialogTitle>
                    <DialogDescription>
                        Choose who can join {room.channelId} and read its chat history.
                    </DialogDescription>
                </DialogHeader>
                <div className="flex items-center justify-between gap-4 rounded-md border border-border p-3">
                    <div>
                        <Label htmlFor="restricted">Invite only</Label>
                        <p className="text-xs text-muted-foreground">
                            Only you, admins and invited people can join. Chat history follows the same rule.
                        </p>
                    </div>
                    <Switch
                        id="restricted"
                        checked={room.restricted}
                        disabled={saving}
                        onCheckedChange={(restricted) => onSave({ restricted, members: room.members })}
                    />
                </div>
                <div className="flex flex-col gap-1">
                    <h3 className="text-sm font-medium">Invited ({room.members.length})</h3>
                    {room.members.length === 0 && <p className="text-sm text-muted-foreground">No one is invited.</p>}
                    <ul className="flex max-h-48 list-none flex-col gap-0.5 overflow-y-auto">
                        {room.members.map((member) => (
                            <MemberRow
                                key={member}
                                name={member}
                                disabled={saving}
                                onRemove={() =>
                                    onSave({ restricted: room.restricted, members: removeMember(room.members, member) })
                                }
                            />
                        ))}
                    </ul>
                </div>
                <form onSubmit={handleAdd} className="flex gap-2">
                    <Input
                        type="text"
                        aria-label="Username to invite"
                        value={input}
                        placeholder="username"
                        onChange={(event) => setInput(event.target.value)}
                        aria-invalid={inputError !== null}
                    />
                    <Button type="submit" disabled={saving}>
                        Invite
                    </Button>
                </form>
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
};
