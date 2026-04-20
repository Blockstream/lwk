export interface TranscriptEntry {
  id: string;
  direction: "outbound" | "inbound";
  method: string;
  rpcId: string;
  topic: string | null;
  timestampMs: number;
  elapsedMs: number | null;
  payload: unknown;
  status: "ok" | "error";
}

let transcriptSequence = 0;

export function createTranscriptEntry(
  input: Omit<TranscriptEntry, "id">,
): TranscriptEntry {
  transcriptSequence += 1;
  return {
    id: `transcript-${String(transcriptSequence)}`,
    ...input,
  };
}

export function prependTranscript(
  entries: TranscriptEntry[],
  entry: TranscriptEntry,
  limit = 80,
): TranscriptEntry[] {
  return [entry, ...entries].slice(0, limit);
}

export function formatTranscriptPayload(payload: unknown): string {
  return JSON.stringify(
    payload,
    (_key, candidate) =>
      typeof candidate === "bigint" ? candidate.toString() : candidate,
    2,
  );
}

export function formatTranscriptTimestamp(entry: TranscriptEntry): string {
  return new Date(entry.timestampMs).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
