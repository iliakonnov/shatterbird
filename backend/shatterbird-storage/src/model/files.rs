use crate::ts;
use mongo_model::{Id, Model};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::EnumTryAs;
use ts_rs::TS;

/// Содержимое отдельной строки текстового файла
#[derive(Debug, Clone, Serialize, Deserialize, Model, TS)]
#[mongo_model(collection = "lines")]
#[ts(export)]
pub struct Line {
    /// Идентификатор объекта в базе данных
    #[ts(as = "ts::Id<Self>")]
    #[serde(rename = "_id")]
    pub id: Id<Self>,

    /// Текст строки
    pub text: String,
}

/// Описание подстроки в файле
#[derive(Debug, Clone, Serialize, Deserialize, Model, TS)]
#[mongo_model(collection = "ranges")]
#[ts(export)]
pub struct Range {
    /// Идентификатор объекта в базе данных
    #[ts(as = "ts::Id<Self>")]
    #[serde(rename = "_id")]
    pub id: Id<Self>,

    /// Идентификатор строки, в которой находится подстрока
    #[ts(as = "ts::Id<Line>")]
    pub line_id: Id<Line>,

    /// Полный путь к этому файлу
    // TODO: Move out of Range to reduce storage costs
    #[ts(as = "Vec<ts::Id<Line>>")]
    pub path: Vec<Id<Node>>,

    /// Индекс первого символа подстроки
    pub start: u32,

    /// Индекс конца подстроки
    pub end: u32,
}

/// Содержимое файла, который не удалось разделить на отдельные строки
#[derive(Debug, Clone, Serialize, Deserialize, Model, TS)]
#[mongo_model(collection = "blobs")]
#[ts(export)]
pub struct BlobFile {
    /// Идентификатор объекта в базе данных
    #[ts(as = "ts::Id<Self>")]
    #[serde(rename = "_id")]
    pub id: Id<Self>,

    /// Содержимое файла
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumTryAs)]
#[ts(export)]
pub enum FileContent {
    /// Символическая ссылка
    Symlink {
        /// Путь, на который ссылается эта ссылка
        target: String,
    },

    /// Директория
    Directory {
        /// Объекты. входящие в эту директорию и их имена
        #[ts(as = "HashMap<String, ts::Id<Node>>")]
        children: HashMap<String, Id<Node>>,
    },

    /// Текстовый файл
    Text {
        /// Суммарный размер файла
        size: u64,

        /// Список строк, входящих в этот файл
        #[ts(as = "Vec<ts::Id<Line>>")]
        lines: Vec<Id<Line>>,
    },

    /// Файл, который не удалось разделить на строки и проанализировать
    Blob {
        /// Суммарный размер файла
        size: u64,

        /// Идентификатор объекта, содержащего этот файл
        #[ts(as = "ts::Id<BlobFile>")]
        content: Id<BlobFile>,
    },
}

/// Объект в файловом дереве
#[derive(Debug, Clone, Serialize, Deserialize, Model, TS)]
#[mongo_model(collection = "nodes")]
#[ts(export)]
pub struct Node {
    /// Идентификатор объекта в базе данных
    #[ts(as = "ts::Id<Self>")]
    #[serde(rename = "_id")]
    pub id: Id<Self>,

    /// Хранит хэш, который используется для идентификации соответствующего объекта в Git
    #[ts(as = "String")]
    #[serde(with = "crate::serializers::gix_hash")]
    pub oid: gix_hash::ObjectId,

    // TODO: Preserve mtime, ctime

    /// Содержимое объекта, в зависимости от его типа
    #[ts(inline)]
    pub content: FileContent,
}

/// Объект коммита, импортированного из Git-репозитория
#[derive(Debug, Clone, Serialize, Deserialize, Model, TS)]
#[mongo_model(collection = "commits")]
#[ts(export)]
pub struct Commit {
    /// Идентификатор объекта в базе данных
    #[ts(as = "ts::Id<Self>")]
    #[serde(rename = "_id")]
    pub id: Id<Self>,

    /// Хранит хэш, который используется для идентификации соответствующего объекта в Git
    #[ts(as = "String")]
    #[serde(rename = "oid", with = "crate::serializers::gix_hash")]
    pub oid: gix_hash::ObjectId,

    /// Идентификатор корневой директории репозитория
    #[ts(as = "ts::Id<Node>")]
    pub root: Id<Node>,

    /// Список коммитов-родителей, если они также загружены в хранилище
    #[ts(as = "Vec<ts::Id<Commit>>")]
    pub parents: Vec<Id<Commit>>,
}
