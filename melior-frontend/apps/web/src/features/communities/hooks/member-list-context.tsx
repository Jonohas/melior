import { createContext, type ReactNode, useCallback, useContext, useState } from 'react';
import { loadMemberListOpen, saveMemberListOpen } from '@/features/communities/api';

export type MemberListToggle = {
    open: boolean;
    toggle: () => void;
};

const MemberListContext = createContext<MemberListToggle>({ open: false, toggle: () => undefined });

/** Whether the server's member list is shown; the channel header's button toggles it. */
export const useMemberList = (): MemberListToggle => useContext(MemberListContext);

/** Holds the member list's open state for a server's pages, remembered across visits. */
export const MemberListProvider = ({ children }: { children: ReactNode }) => {
    const [open, setOpen] = useState(loadMemberListOpen);
    const toggle = useCallback(() => {
        saveMemberListOpen(!open);
        setOpen(!open);
    }, [open]);
    return <MemberListContext.Provider value={{ open, toggle }}>{children}</MemberListContext.Provider>;
};
