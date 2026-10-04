export interface FileChange {
  filename: string;
  status: string;
}

export type ListFiles = (params: {
  owner: string;
  repo: string;
  pull_number: number;
}) => Promise<FileChange[]>;

/**
 * The PR's changed files, by new name. Removed files are excluded — a deleted
 * file has no node in the graph, and `reverse_reach` ignores unknown paths
 * anyway, but excluding them keeps the request honest about what was asked.
 */
export async function getChangedFiles(
  listFiles: ListFiles,
  owner: string,
  repo: string,
  pr: number,
): Promise<string[]> {
  const changes = await listFiles({ owner, repo, pull_number: pr });
  return changes
    .filter((c) => c.status !== 'removed' && c.filename.length > 0)
    .map((c) => c.filename);
}
