use std::sync::Arc;
use std::time::Duration;
use std::borrow::Cow;
use std::{fmt, fs, io};

use screeps_api::{RoomName, TerrainGrid};
use screeps_api::data::room_name::RoomNameAbsoluteCoordinates;
use futures_cpupool::CpuPool;
use futures::{future, stream, Future, Stream};
use tokio_core::reactor;

use {app_dirs, bincode, sled, time};

static APP_DESC: app_dirs::AppInfo = app_dirs::AppInfo {
    author: "OpenScreeps",
    name: "screeps-rs",
};

// TODO: cache per server connection.
const OLD_DB_FILE_NAME: &'static str = "cache";

const DB_FILE_NAME: &'static str = "cache-v0.2";

#[inline(always)]
fn keep_terrain_for() -> time::Duration {
    time::Duration::days(1)
}

mod errors {
    use std::{fmt, io};
    use app_dirs;

    #[derive(Debug)]
    pub enum CreationError {
        DirectoryCreation(io::Error),
        CacheDir(app_dirs::AppDirsError),
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
                //CreationError::Database(ref e) => write!(f, "error opening the cache database: {}", e),
            }
        }
    }
}

// placeholder for if we find what the sled database errors are.
pub enum DbError {}
impl fmt::Display for DbError {
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        match *self {}
    }
}

pub use self::errors::CreationError;

#[derive(Clone)]
pub struct Cache {
    database: Arc<sled::Tree>,
    access_pool: CpuPool,
}

impl Cache {
    pub fn load() -> Result<Self, CreationError> {
        let mut path = app_dirs::get_app_root(app_dirs::AppDataType::UserCache, &APP_DESC)?;

        fs::create_dir_all(&path)?;

        path.push(OLD_DB_FILE_NAME);

        if let Err(e) = fs::remove_dir_all(&path) {
            if e.kind() != io::ErrorKind::NotFound {
                warn!(
                    "error deleting old cache directory ({}): {}",
                    path.display(),
                    e
                );
            }
        }

        path.pop();

        path.push(DB_FILE_NAME);

        debug!("Opening cache from file {}", path.display());

        // TODO: figure out what error messages sled can panic with, and clear data when that happens.
        let database = sled::Config::default()
            .path(path.to_str().expect(
                "screeps-rs (with sled) currently doesn't handle non-unicode path names. expected unicode path name.",
            ).to_owned())
            .tree();

        Ok(Cache {
            database: Arc::new(database),
            access_pool: CpuPool::new(3),
        })
    }

    pub fn start_cache_clean_task(&self, handle: &reactor::Handle) -> io::Result<()> {
        let pool = self.access_pool.clone();
        let db = self.database.clone();
        // run once on app startup, then once every hour.
        let stream = stream::once(Ok(())).chain(reactor::Interval::new(
            Duration::from_secs(60 * 60),
            handle,
        )?);

        handle.spawn(
            stream
                .then(move |result| {
                    if let Err(e) = result {
                        warn!("error with cache cleanup interval: {}", e);
                    }

                    let db = db.clone();

                    pool.spawn_fn(move || {
                        cleanup_database(&db);
                        future::ok(())
                    })
                })
                .fold((), |(), _| future::ok(())),
        );

        Ok(())
    }

    pub fn set_terrain(
        &self,
        server: &str,
        shard: Option<&str>,
        room: RoomName,
        data: &TerrainGrid,
    ) -> impl Future<Item = (), Error = DbError> {
        let key = ShardCacheKey::terrain(server, shard, room).encode();

        let to_store = CacheEntry {
            fetched: time::get_time(),
            data: data,
        };

        let value = bincode::serialize(&to_store, bincode::Infinite)
            .expect("expected serializing data using bincode to unequivocally succeed.");

        let sent_database = self.database.clone();

        self.access_pool
            .spawn_fn(move || Ok(sent_database.set(key, value)))
    }

    pub fn get_terrain(
        &self,
        server: &str,
        shard: Option<&str>,
        room: RoomName,
    ) -> impl Future<Item = Option<TerrainGrid>, Error = DbError> {
        let key = ShardCacheKey::terrain(server, shard, room).encode();

        let sent_database = self.database.clone();

        self.access_pool.spawn_fn(move || {
            let parsed = match sent_database.get(&key) {
                Some(db_vector) => {
                    match bincode::deserialize_from::<_, CacheEntry<_>, _>(&mut &*db_vector, bincode::Infinite) {
                        Ok(v) => Some(v.data),
                        Err(e) => {
                            warn!(
                                "cache database entry found corrupted.\
                                 \nEntry: (terrain:{})\
                                 \nDecode error: {}\
                                 \nRemoving data.",
                                room, e
                            );

                            sent_database.del(&key);

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

fn cleanup_database(db: &sled::Tree) {
    let to_remove = db.iter()
        .filter_map(|(key, value)| {
            let parsed_key = match ShardCacheKey::decode(&key) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "when clearing old cache: unknown key '{:?}' found (read error: {}). Deleting.",
                        key, e
                    );
                    return Some(key);
                }
            };

            let now = time::get_time();

            let keep_result = match parsed_key.key {
                CacheKeyInner::Terrain(_) => bincode::deserialize::<CacheEntry<TerrainGrid>>(&value)
                    .map(|entry| now - entry.fetched < keep_terrain_for()),
            };

            match keep_result {
                Ok(true) => {
                    trace!("keeping cache entry ({:?})", parsed_key);
                    None
                }
                Ok(false) => {
                    debug!("removing cache entry ({:?}): old data.", parsed_key);
                    Some(key)
                }
                Err(e) => {
                    debug!(
                        "removing cache entry ({:?}): invalid data ({})",
                        parsed_key, e
                    );
                    Some(key)
                }
            }
        })
        .collect::<Vec<_>>();

    for key in to_remove {
        db.del(&key);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    #[serde(with = "timespec_serialize_seconds")] fetched: time::Timespec,
    data: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CacheKeyInner {
    // NOTE: whenever adding a variant, the length return in 'encode' must be tested and updated.
    Terrain(RoomNameAbsoluteCoordinates),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ShardCacheKey<'a> {
    server: Cow<'a, str>,
    shard: Option<Cow<'a, str>>,
    key: CacheKeyInner,
}

impl ShardCacheKey<'static> {
    fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

impl<'a> ShardCacheKey<'a> {
    fn terrain<T, U>(server: T, shard: Option<U>, room_name: RoomName) -> Self
    where
        T: Into<Cow<'a, str>>,
        U: Into<Cow<'a, str>>,
    {
        ShardCacheKey {
            server: server.into(),
            shard: shard.map(Into::into),
            key: CacheKeyInner::Terrain(room_name.into()),
        }
    }

    /// Returns bytes representing this cache key, encoded using `bincode`.
    fn encode(&self) -> Vec<u8> {
        bincode::serialize(self, bincode::Infinite)
            .expect("expected writing cache key with infinite size to be within that infinite size.")
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
