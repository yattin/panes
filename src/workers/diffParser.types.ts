import type { ParsedLine } from "../contexts/git/domain/parseDiff";

export interface DiffParseWorkerRequest {
  id: number;
  raw: string;
}

export interface DiffParseWorkerResponse {
  id: number;
  parsed: ParsedLine[];
  filename: string | null;
  adds: number;
  dels: number;
}
