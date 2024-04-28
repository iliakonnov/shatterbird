// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { BlobFile } from "./BlobFile";
import type { Id } from "./Id";
import type { Line } from "./Line";
import type { Node } from "./Node";

export type FileContent = { "Symlink": { target: string, } } | { "Directory": { children: { [key: string]: Id<Node> }, } } | { "Text": { size: bigint, lines: Array<Id<Line>>, } } | { "Blob": { size: bigint, content: Id<BlobFile>, } };