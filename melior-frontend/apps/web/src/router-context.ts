import type { QueryClient } from '@tanstack/react-query';
import type { AuthContext } from '@/lib/auth';

export type RouterContext = {
    queryClient: QueryClient;
    /** Absent outside the `_authenticated` layout: that route's `beforeLoad` is what puts it here. */
    auth?: AuthContext;
};
