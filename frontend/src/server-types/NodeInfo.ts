// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { Id } from "./Id";
import type { Node } from "./Node";

export type NodeInfo = { 
/**
 * Идентифкатор этого узла в базе данных
 */
_id: Id<Node>, 
/**
 * Тип узла
 */
kind: "Symlink" | "Directory" | "Text" | "Blob", };