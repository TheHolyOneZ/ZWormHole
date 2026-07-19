

export interface FolderNode {
  name: string;

  rel_path: string;
  is_dir: boolean;

  size: number;
  children: FolderNode[];
}
