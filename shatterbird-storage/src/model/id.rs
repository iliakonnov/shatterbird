use super::Model;
use derive_where::{derive_where, DeriveWhere};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

#[derive(Serialize, Deserialize, DeriveWhere)]
#[derive_where(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[serde(transparent)]
#[allow(private_bounds)]
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
