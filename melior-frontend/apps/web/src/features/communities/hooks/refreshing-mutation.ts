import { type QueryKey, type UseMutationResult, useMutation, useQueryClient } from '@tanstack/react-query';

/**
 * A mutation that refetches the given queries once it succeeded: the server's answer is the truth, and the
 * community feed's `structure_changed` would refetch them anyway, just later.
 */
export const useRefreshingMutation = <TVariables, TResult = void>(
    mutationFn: (variables: TVariables) => Promise<TResult>,
    keys: readonly QueryKey[],
): UseMutationResult<TResult, Error, TVariables> => {
    const queryClient = useQueryClient();
    return useMutation({
        mutationFn,
        onSuccess: async () => {
            await Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })));
        },
    });
};
