mod memory;
pub mod persistence;

pub use self::persistence::{
    PersistenceStorageOperation, PersistenceStorageReadAndWrite, PersistenceStorageRemove,
};
pub use memory::MemStorage;

#[cfg(feature = "wasm")]
pub use self::persistence::idb::IDBStorage as Storage;

#[cfg(feature = "default")]
pub use self::persistence::kv::KvStorage as Storage;
