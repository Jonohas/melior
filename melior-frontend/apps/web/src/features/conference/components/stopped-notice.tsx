import { Lock } from 'lucide-react';
import type { ReactNode } from 'react';
import { StatePage } from '@/components';
import { describeStop, type StopReason } from '@/features/conference/domain';

type StoppedNoticeProps = {
    reason: StopReason;
    /** Where to go next, for example back to the server. */
    actions: ReactNode;
};

/** The call is closed to you: the page says why, and where to go next. */
export const StoppedNotice = ({ reason, actions }: StoppedNoticeProps) => (
    <StatePage alert icon={Lock} title={describeStop(reason)} actions={actions}>
        The call and its chat are no longer shown to you.
    </StatePage>
);
