use crate::Model;
use bson::oid::ObjectId;
use bson::Bson;
use derive_where::{derive_where, DeriveWhere};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

#[derive(Serialize, Deserialize, DeriveWhere)]
#[derive_where(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[serde(transparent)]
pub struct Id<T: Model + ?Sized> {
    pub id: ObjectId,
    #[derive_where(skip)]
    pub _phantom: PhantomData<fn() -> T>,
}

impl<T: Model + ?Sized> Id<T> {
    pub fn new() -> Self {
        Id {
            id: ObjectId::new(),
            _phantom: Default::default(),
        }
    }
}

impl<T: Model + ?Sized> Display for Id<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", T::COLLECTION, self.id)
    }
}

impl<T: Model + ?Sized> From<ObjectId> for Id<T> {
    fn from(id: ObjectId) -> Self {
        Id {
            id,
            _phantom: PhantomData,
        }
    }
}

impl<T: Model + ?Sized> From<Id<T>> for ObjectId {
    fn from(id: Id<T>) -> Self {
        id.id
    }
}

impl<T: Model + ?Sized> From<Id<T>> for Bson {
    fn from(id: Id<T>) -> Self {
        id.id.into()
    }
}

impl<T: Model + ?Sized> FromStr for Id<T> {
    type Err = bson::oid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ObjectId::from_str(s).map(|id| id.into())
    }
}
