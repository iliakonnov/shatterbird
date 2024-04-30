// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { BlobFile } from "./BlobFile";
import type { Id } from "./Id";
import type { Line } from "./Line";
import type { NodeInfo } from "./NodeInfo";

export type ExpandedFileContent = { "Symlink": { target: string, } } | { "Directory": { children: { [key: string]: NodeInfo }, } } | { "Text": { size: bigint, lines: Array<Line>, } } | { "Blob": { size: bigint, content: Id<BlobFile>, } };