use std::marker::PhantomData;
use ts_rs::TS;

#[derive(TS)]
#[ts(export)]
pub struct Id<T: TS + ?Sized> {
    #[ts(rename = "$oid")]
    pub id: String,
    #[ts(skip)]
    pub _phantom: PhantomData<fn() -> T>,
}
