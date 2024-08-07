// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { Id } from "./Id";
import type { Node } from "./Node";

/**
 * Объект коммита, импортированного из Git-репозитория
 */
export type Commit = { 
/**
 * Идентификатор объекта в базе данных
 */
_id: Id<Commit>, 
/**
 * Хранит хэш, который используется для идентификации соответствующего объекта в Git
 */
oid: string, 
/**
 * Идентификатор корневой директории репозитория
 */
root: Id<Node>, 
/**
 * Список коммитов-родителей, если они также загружены в хранилище
 */
parents: Array<Id<Commit>>, };