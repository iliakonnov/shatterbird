import {FilePermission, FileStat, FileSystemError, FileType} from "vscode";
import {Node, DirectoryLike} from "./types.ts";
import FsClient from "./fsClient.ts";
import {Commit} from "../server-types/Commit.ts";
import {DirNode} from "./node.ts";

export default class RepoRoot implements DirectoryLike {
    public readonly commit: Commit;
    public readonly name: string;
    public readonly fileType = FileType.Directory;
    private readonly client: FsClient;

    constructor(client: FsClient, commit: Commit) {
        this.client = client;
        this.commit = commit;
        this.name = commit.oid;
    }

    async getChildren(): Promise<Node[]> {
        const root = new DirNode(this.client, this.commit.root, this.name);
        return await root.getChildren();
    }

    async get(child: string): Promise<Node | null> {
        const root = new DirNode(this.client, this.commit.root, this.name);
        return await root.get(child);
    }

    async getStat(): Promise<FileStat> {
        return {
            ctime: 0,
            mtime: 0,
            permissions: FilePermission.Readonly,
            size: 0,
            type: FileType.Directory
        }
    }
}