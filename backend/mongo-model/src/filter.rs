use std::marker::PhantomData;

use bson::{doc, Document};
use serde::{Deserialize, Serialize};

pub struct Root<T, F> {
    pub this: PhantomData<T>,
    pub filter: F,
}

pub struct Proof<P>(PhantomData<P>);

impl<P> From<P> for Proof<P> {
    fn from(_: P) -> Self {
        Proof(PhantomData)
    }
}

pub struct Access<F, P> {
    pub field: &'static str,
    pub proof: Proof<P>,
    pub filter: F,
}

#[derive(Serialize, Deserialize)]
pub enum Value<Val> {
    #[serde(rename = "$eq")]
    Eq(Val),
    #[serde(rename = "$gt")]
    Gt(Val),
    #[serde(rename = "$gte")]
    Gte(Val),
    #[serde(rename = "$in")]
    In(Vec<Val>),
    #[serde(rename = "$lt")]
    Lt(Val),
    #[serde(rename = "$lte")]
    Lte(Val),
    #[serde(rename = "$ne")]
    Ne(Val),
    #[serde(rename = "$nin")]
    Nin(Vec<Val>),
    #[serde(untagged)]
    Value(Val),
}

trait Filterable<T> {
    type Value;

    fn filterable(self) -> Option<(String, bson::ser::Result<Document>)>;
}

impl<T, F> Root<T, F> {
    fn new(filter: F) -> Self {
        Self {
            this: Default::default(),
            filter,
        }
    }
}

impl<T, F: Filterable<T>> Filterable<T> for Root<T, F> {
    type Value = F::Value;

    fn filterable(self) -> Option<(String, bson::ser::Result<Document>)> {
        self.filter.filterable()
    }
}

impl<T, F, P, V> Filterable<T> for Access<F, P>
where
    F: Filterable<V>,
    P: FnOnce(T) -> V,
{
    type Value = V;

    fn filterable(self) -> Option<(String, bson::ser::Result<Document>)> {
        let (f, doc) = self.filter.filterable()?;
        if f.is_empty() {
            Some((self.field.to_string(), doc))
        } else {
            Some((format!("{}.{}", self.field, f), doc))
        }
    }
}

impl<Val: Serialize> Filterable<Val> for Value<Val> {
    type Value = Val;

    fn filterable(self) -> Option<(String, bson::ser::Result<Document>)> {
        Some((String::new(), bson::to_document(&self)))
    }
}

pub trait Filter<T> {
    fn build(self) -> bson::ser::Result<Document>;
}

#[allow(private_bounds)]
impl<T, F> Filter<T> for Root<T, F>
where
    Self: Filterable<T>,
{
    fn build(self) -> bson::ser::Result<Document> {
        let (field, doc) = match self.filterable() {
            Some(x) => x,
            None => return Ok(Document::new()),
        };
        let mut result = Document::new();
        _ = result.insert(field, doc?);
        Ok(result)
    }
}

macro_rules! impl_for_tuple {
    ($($i:ident),*) => {
        #[allow(private_bounds)]
        impl<T, $($i),*> Filter<T> for ($($i,)*)
        where
            $($i: Filterable<T>),*
        {
            fn build(self) -> bson::ser::Result<Document> {
                let ($($i,)*) = self;
                let mut result = Document::new();
                $(
                    if let Some((field, doc)) = $i.filterable() {
                        let existing = result.insert(field, doc?);
                        assert!(existing.is_none());
                    }
                )*
                Ok(result)
            }
        }
    };
}

impl_for_tuple!(T1);
impl_for_tuple!(T1, T2);
impl_for_tuple!(T1, T2, T3);
impl_for_tuple!(T1, T2, T3, T4);

#[macro_export]
macro_rules! filter {
    ($( $ty:ty { $($rest:tt)* } $(,)? )*) => {
        ($(
            $crate::filter::Root::<$ty, _> {
                this: Default::default(),
                filter: $crate::filter!(@filter $ty { $($rest)* })
            },
        )*)
    };

    (@filter $(:)? == $val:expr) => { $crate::filter::Value::Eq($val) };
    (@filter $(:)? != $val:expr) => { $crate::filter::Value::Neq($val) };
    (@filter $(:)? < $val:expr) => { $crate::filter::Value::Lt($val) };
    (@filter $(:)? <= $val:expr) => { $crate::filter::Value::Lte($val) };
    (@filter $(:)? > $val:expr) => { $crate::filter::Value::Gt($val) };
    (@filter $(:)? >= $val:expr) => { $crate::filter::Value::Gte($val) };
    (@filter $(:)? in $val:expr) => { $crate::filter::Value::In($val) };
    (@filter $(:)? not in $val:expr) => { $crate::filter::Value::Nin($val) };
    (@filter $(:)? === $val:expr) => { $crate::filter::Value::Value($val) };

    (@filter $(:)? $ty:ty { $field:ident [$name:literal] $($rest:tt)* } ) => {
        $crate::filter::Access {
            field: $name,
            proof: (|x: $ty| x.$field).into(),
            filter: $crate::filter!(@filter $($rest)*)
        }
    };
    (@filter $(:)? $ty:ty { $field:ident $($rest:tt)* } ) => {
        $crate::filter::Access {
            field: stringify!($field),
            proof: (|x: $ty| x.$field).into(),
            filter: $crate::filter!(@filter $($rest)*)
        }
    };
}

#[cfg(test)]
mod tests {
    use bson::doc;

    use super::Filter;
    use super::{Access, Root, Value};

    struct Foo {
        bar: Bar,
    }

    struct Bar {
        x: i32,
        y: i32,
    }

    #[test]
    fn plain() {
        let filter = Root::<Foo, _> {
            this: Default::default(),
            filter: Access {
                field: "bar",
                proof: (|foo: Foo| foo.bar).into(),
                filter: Access {
                    field: "x",
                    proof: (|bar: Bar| bar.x).into(),
                    filter: Value::<i32>::Eq(123),
                },
            },
        };
        let doc = filter.build().unwrap();
        assert_eq!(doc, doc! { "bar.x": { "$eq": 123 } });
    }

    #[test]
    fn with_macro() {
        let filter = filter!(Foo { bar: Bar { x == 123 }});
        let doc = filter.build().unwrap();
        assert_eq!(doc, doc! { "bar.x": { "$eq": 123 } });
    }

    #[test]
    fn many() {
        let filter = filter!(
            Foo { bar: Bar { x == 123 }}
            Foo { bar: Bar { y == 456 }}
        );
        let doc = filter.build().unwrap();
        assert_eq!(
            doc,
            doc! {
                "bar.x": { "$eq": 123 },
                "bar.y": { "$eq": 456 }
            }
        );
    }
}
