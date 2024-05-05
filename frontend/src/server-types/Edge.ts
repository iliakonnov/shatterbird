// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { Id } from "./Id";
import type { Vertex } from "./Vertex";

/**
 * Ребро графа, связывающее те или иные узлы
 */
export type Edge = { 
/**
 * Идентификатор объекта в базе данных
 */
_id: Id<Edge>, 
/**
 * Информация об ребре, предоставленная LSIF
 */
data: { 
/**
 * Вид ребра
 */
edge: "Contains" | "Moniker" | "NextMoniker" | "Next" | "PackageInformation" | "Item" | "Definition" | "Declaration" | "Hover" | "References" | "Implementation" | "TypeDefinition" | "FoldingRange" | "DocumentLink" | "DocumentSymbol" | "Diagnostic", 
/**
 * Входящий узел, если один
 */
in_v?: Id<Vertex>, 
/**
 * Входящие узлы, если несколько
 */
in_vs?: Array<Id<Vertex>>, 
/**
 * Исходящий узел
 */
out_v: Id<Vertex>, }, };