use std::time::Duration;
use std::borrow::Cow;
use std::{fs, io};

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
    use {app_dirs, sled};

    #[derive(Debug)]
    pub enum CreationError {
        DirectoryCreation(io::Error),
        DatabaseDeletion(io::Error),
        CacheDir(app_dirs::AppDirsError),
        Sled(sled::Error<()>),
    }

    impl CreationError {
        pub fn directory_creation(e: io::Error) -> Self {
            CreationError::DirectoryCreation(e)
        }

        pub fn database_deletion(e: io::Error) -> Self {
            CreationError::DatabaseDeletion(e)
        }
    }

    impl From<app_dirs::AppDirsError> for CreationError {
        fn from(e: app_dirs::AppDirsError) -> Self {
            CreationError::CacheDir(e)
        }
    }

    impl From<sled::Error<()>> for CreationError {
        fn from(e: sled::Error<()>) -> Self {
            CreationError::Sled(e)
        }
    }

    impl fmt::Display for CreationError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match *self {
                CreationError::DirectoryCreation(ref e) => write!(f, "error creating cache directory: {}", e),
                CreationError::DatabaseDeletion(ref e) => write!(f, "error deleting corrupted cache database: {}", e),
                CreationError::CacheDir(ref e) => write!(f, "error finding cache directory: {}", e),
                CreationError::Sled(ref e) => write!(f, "sled database error: {:?}", e),
            }
        }
    }
}

pub use self::errors::CreationError;

#[derive(Clone)]
pub struct Cache {
    database: sled::Tree,
    access_pool: CpuPool,
}

impl Cache {
    pub fn load() -> Result<Self, CreationError> {
        let mut path = app_dirs::get_app_root(app_dirs::AppDataType::UserCache, &APP_DESC)?;

        fs::create_dir_all(&path).map_err(CreationError::directory_creation)?;

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

        let config = sled::ConfigBuilder::default().path(&path).build();

        let database_result = match sled::Tree::start(config.clone()) {
            Err(sled::Error::Corruption { .. }) => {
                warn!("deleting corrupted database: {}", path.display());
                fs::remove_file(path).map_err(CreationError::database_deletion)?;
                sled::Tree::start(config)
            }
            x => x,
        };

        let database = database_result?;

        Ok(Cache {
            database: database,
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
                        warn!("error with cache cleanup interval: {:?}", e);
                    }

                    let db = db.clone();

                    pool.spawn_fn(move || {
                        let result = cleanup_database(&db);
                        if let Err(e) = result {
                            warn!("error cleaning up database file: {:?}", e);
                        }

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
    ) -> impl Future<Item = (), Error = sled::Error<()>> {
        let key = ShardCacheKey::terrain(server, shard, room).encode();

        let to_store = CacheEntry {
            fetched: time::get_time(),
            data: data,
        };

        let value = bincode::serialize(&to_store, bincode::Infinite)
            .expect("expected serializing data using bincode to unequivocally succeed.");

        let sent_database = self.database.clone();

        self.access_pool
            .spawn_fn(move || sent_database.set(key, value))
    }

    pub fn get_terrain(
        &self,
        server: &str,
        shard: Option<&str>,
        room: RoomName,
    ) -> impl Future<Item = Option<TerrainGrid>, Error = sled::Error<()>> {
        let key = ShardCacheKey::terrain(server, shard, room).encode();

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
                                room, e
                            );

                            sent_database.del(&key)?;

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

fn cleanup_database(db: &sled::Tree) -> Result<(), sled::Error<()>> {
    let to_remove = db.iter()
        .filter_map(|result| {
            let (key, value) = match result {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };

            let parsed_key = match ShardCacheKey::decode(&key) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "when clearing old cache: unknown key '{:?}' found (read error: {}). Deleting.",
                        key, e
                    );
                    return Some(Ok(key));
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
                    Some(Ok(key))
                }
                Err(e) => {
                    debug!(
                        "removing cache entry ({:?}): invalid data ({})",
                        parsed_key, e
                    );
                    Some(Ok(key))
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    for key in to_remove {
        db.del(&key)?;
    }

    Ok(())
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
