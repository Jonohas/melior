import { QueryClientProvider } from '@tanstack/react-query';
import { createRootRouteWithContext, HeadContent, Scripts, useRouter } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import type { RouterContext } from '@/router-context';
import appCss from '../styles.css?url';

export const Route = createRootRouteWithContext<RouterContext>()({
    ssr: false,
    head: () => ({
        meta: [
            { charSet: 'utf-8' },
            { name: 'viewport', content: 'width=device-width, initial-scale=1' },
            { title: 'Melior' },
        ],
        links: [
            { rel: 'stylesheet', href: appCss },
            { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
        ],
    }),
    shellComponent: RootDocument,
});

function RootDocument({ children }: { children: ReactNode }) {
    const router = useRouter();
    const { queryClient } = router.options.context;

    return (
        <html lang="en">
            <head>
                <HeadContent />
            </head>
            <body>
                <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
                <Scripts />
            </body>
        </html>
    );
}
