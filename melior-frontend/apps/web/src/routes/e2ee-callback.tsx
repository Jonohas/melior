import { createFileRoute } from '@tanstack/react-router';
import { ShieldCheck } from 'lucide-react';
import { useEffect } from 'react';
import { StatePage } from '@/components';
import { completeDeviceVouching } from '@/features/conference/api';

/**
 * Where the realm sends the browser back after vouching for this device (see `e2ee-vouch-login.ts`). It keeps the
 * device's token and returns to the page the call was opened on. Outside the authenticated layout: nothing here
 * needs the session, and the page is gone again at once.
 */
export const Route = createFileRoute('/e2ee-callback')({
    component: DeviceVouchingCallback,
});

function DeviceVouchingCallback() {
    useEffect(() => {
        void completeDeviceVouching(window.location.search).then((back) => window.location.replace(back));
    }, []);
    return (
        <StatePage standalone icon={ShieldCheck} title="Verifying this device">
            Taking you back to the call…
        </StatePage>
    );
}
