import {FileStat, FileType} from "vscode";

export interface BaseNode {
    readonly name: string;
    readonly fileType: FileType;

    getStat(): Promise<FileStat>
}

export interface DirectoryLike extends BaseNode {
    readonly fileType: FileType.Directory;

    getChildren(): Promise<Node[]>
    get(child: string): Promise<Node | null>
}

export interface FileLike extends BaseNode {
    readonly fileType: FileType.File;

    getContent(): Promise<Uint8Array>
}

export interface Symlink extends BaseNode {
    readonly fileType: FileType.SymbolicLink;

    getTarget(): Promise<string>;
}

export type Node = FileLike | DirectoryLike | Symlink;