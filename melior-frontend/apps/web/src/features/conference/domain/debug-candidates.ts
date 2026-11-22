/** The ICE candidate types: on this host, seen by a STUN server, learned from the peer, or a TURN relay. */
export const CANDIDATE_TYPES = ['host', 'srflx', 'prflx', 'relay'] as const;
export type CandidateType = (typeof CANDIDATE_TYPES)[number];

/** How many candidates of each type a connection gathered or received. */
export type CandidateCounts = Record<CandidateType, number>;

export const NO_CANDIDATES: CandidateCounts = { host: 0, srflx: 0, prflx: 0, relay: 0 };

const TYPE_PATTERN = /\styp\s+(\S+)/;

const isCandidateType = (value: string | undefined): value is CandidateType =>
    CANDIDATE_TYPES.some((type) => type === value);

/**
 * The type of an SDP candidate line (`candidate:… typ srflx raddr …`). Null for the empty end-of-candidates
 * marker and for anything without a known type.
 */
export const candidateTypeOf = (candidate: string | undefined): CandidateType | null => {
    const type = candidate?.match(TYPE_PATTERN)?.[1];
    return isCandidateType(type) ? type : null;
};

/** Counts one more candidate; one without a known type changes nothing. */
export const countCandidate = (counts: CandidateCounts, candidate: string | undefined): CandidateCounts => {
    const type = candidateTypeOf(candidate);
    if (type === null) return counts;
    return { ...counts, [type]: counts[type] + 1 };
};
