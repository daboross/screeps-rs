use std::sync::Arc;
use std::time::Duration;
use std::{fs, io};

use screeps_api::{RoomName, TerrainGrid};
use screeps_api::data::room_name::RoomNameAbsoluteCoordinates;
use futures_cpupool::CpuPool;
use futures::{future, stream, Future, Stream};
use tokio_core::reactor;

use {app_dirs, bincode, rocksdb, time};

static APP_DESC: app_dirs::AppInfo = app_dirs::AppInfo {
    author: "OpenScreeps",
    name: "screeps-rs",
};

// TODO: cache per server connection.
const DB_FILE_NAME: &'static str = "cache";

#[inline(always)]
fn keep_terrain_for() -> time::Duration {
    time::Duration::days(1)
}

mod errors {
    use std::{fmt, io};
    use {app_dirs, rocksdb};

    #[derive(Debug)]
    pub enum CreationError {
        DirectoryCreation(io::Error),
        CacheDir(app_dirs::AppDirsError),
        Database(rocksdb::Error),
    }

    impl From<rocksdb::Error> for CreationError {
        fn from(e: rocksdb::Error) -> Self {
            CreationError::Database(e)
        }
    }

    impl From<app_dirs::AppDirsError> for CreationError {
        fn from(e: app_dirs::AppDirsError) -> Self {
            CreationError::CacheDir(e)
        }
    }
    impl From<io::Error> for CreationError {
        fn from(e: io::Error) -> Self {
            CreationError::DirectoryCreation(e)
        }
    }

    impl fmt::Display for CreationError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match *self {
                CreationError::DirectoryCreation(ref e) => write!(f, "error creating cache directory: {}", e),
                CreationError::CacheDir(ref e) => write!(f, "error finding cache directory: {}", e),
                CreationError::Database(ref e) => write!(f, "error opening the cache database: {}", e),
            }
        }
    }
}

pub use self::errors::CreationError;

#[derive(Clone)]
pub struct Cache {
    database: Arc<rocksdb::DB>,
    access_pool: CpuPool,
}

impl Cache {
    pub fn load() -> Result<Self, CreationError> {
        let mut path = app_dirs::get_app_root(app_dirs::AppDataType::UserCache, &APP_DESC)?;

        fs::create_dir_all(&path)?;

        path.push(DB_FILE_NAME);

        debug!("Opening cache from file {}", path.display());

        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        // TODO: anything more than 0 optimizations

        // TODO: learn what error messages rocksdb opens for, for example, a corrupt database.
        // If we can parse the error message, we can delete the database in these cases and then
        // re-initialize it.
        let database = rocksdb::DB::open(&options, path)?;

        Ok(Cache {
            database: Arc::new(database),
            access_pool: CpuPool::new(3),
        })
    }

    pub fn start_cache_clean_task(&self, handle: &reactor::Handle) -> io::Result<()> {
        let pool = self.access_pool.clone();
        let db = self.database.clone();
        // run once on app startup, then once every hour.
        let stream = stream::once(Ok(())).merge(reactor::Interval::new(Duration::from_secs(60 * 60), handle)?);

        handle.spawn(
            stream
                .then(move |result| {
                    if let Err(e) = result {
                        warn!("error with cache cleanup interval: {}", e);
                    }

                    let db = db.clone();

                    pool.spawn_fn(move || {
                        if let Err(e) = cleanup_database(&db) {
                            warn!("error cleaning up cache database: {}", e);
                        }

                        future::ok(())
                    })
                })
                .fold((), |(), _| future::ok(())),
        );

        Ok(())
    }

    pub fn set_terrain(&self, room: RoomName, data: &TerrainGrid) -> impl Future<Item = (), Error = rocksdb::Error> {
        let key = CacheKey::Terrain(room.into()).encode();

        let to_store = CacheEntry {
            fetched: time::get_time(),
            data: data,
        };

        let value = bincode::serialize(&to_store, bincode::Infinite)
            .expect("expected serializing data using bincode to unequivocally succeed.");

        let sent_database = self.database.clone();

        self.access_pool
            .spawn_fn(move || sent_database.put(&key, &value))
    }

    pub fn get_terrain(&self, room: RoomName) -> impl Future<Item = Option<TerrainGrid>, Error = rocksdb::Error> {
        let key = CacheKey::Terrain(room.into()).encode();

        let sent_database = self.database.clone();

        self.access_pool.spawn_fn(move || {
            let parsed = match sent_database.get(&key)? {
                Some(db_vector) => {
                    match bincode::deserialize_from::<_, CacheEntry<_>, _>(&mut &*db_vector, bincode::Infinite) {
                        Ok(v) => Some(v.data),
                        Err(e) => {
                            warn!(
                                "cache database entry found corrupted.\
                                 \nEntry: (terrain:{})\
                                 \nDecode error: {}\
                                 \nRemoving data.",
                                room,
                                e
                            );

                            if let Err(e) = sent_database.delete(&key) {
                                warn!("deleting cache entry (terrain:{}) failed: {}", room, e);

                            }

                            None
                        }
                    }
                }
                None => None,
            };

            Ok(parsed)
        })
    }
}

fn cleanup_database(db: &rocksdb::DB) -> Result<(), rocksdb::Error> {
    let snapshot = rocksdb::Snapshot::new(&db);

    for (key, value) in snapshot.iterator(rocksdb::IteratorMode::Start) {
        let parsed_key = match CacheKey::decode(&key) {
            Ok(v) => v,
            Err(e) => {
                warn!("when clearing old cache: unknown key '{:?}' found (read error: {}). Deleting.", key, e);
                db.delete(&key)?;
                continue;
            }
        };

        let now = time::get_time();

        let keep_result = match parsed_key {
            CacheKey::Terrain(_) => bincode::deserialize::<CacheEntry<TerrainGrid>>(&value)
                .map(|entry| now - entry.fetched < keep_terrain_for()),
        };

        match keep_result {
            Ok(true) => {
                trace!("keeping cache entry ({:?})", parsed_key);
            }
            Ok(false) => {
                debug!("removing cache entry ({:?}): old data.", parsed_key);
                db.delete(&key)?;
            }
            Err(e) => {
                debug!("removing cache entry ({:?}): invalid data ({})", parsed_key, e);
                db.delete(&key)?;
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    #[serde(with = "timespec_serialize_seconds")] fetched: time::Timespec,
    data: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CacheKey {
    // NOTE: whenever adding a variant, the length return in 'encode' must be tested and updated.
    Terrain(RoomNameAbsoluteCoordinates),
}

impl CacheKey {
    fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Returns a byte array representing this cache key, encoded using `bincode`.
    ///
    /// NOTE: the length of the returned array is NOT stable, and will change in the future.
    fn encode(&self) -> [u8; 12] {
        let mut result = [0u8; 12];

        bincode::serialize_into(&mut &mut result[..], self, bincode::Bounded(12))
            .expect("expected writing cache key of known length to array of known length to succeed.");
        result
    }
}

mod timespec_serialize_seconds {
    use time::Timespec;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &Timespec, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(date.sec)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Timespec, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Timespec::new(i64::deserialize(deserializer)?, 0))
    }
}
