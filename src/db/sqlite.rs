use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::params_from_iter;
use rusqlite::types::{FromSql, ToSql, ToSqlOutput, ValueRef};

pub use rusqlite::types::Value;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    BlockingTask(tokio::task::JoinError),
    ConnectionPoisoned,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "{error}"),
            Self::BlockingTask(error) => write!(formatter, "SQLite blocking task failed: {error}"),
            Self::ConnectionPoisoned => write!(formatter, "SQLite connection lock was poisoned"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

#[derive(Debug, Default)]
pub struct Params(Vec<Result<Value, rusqlite::Error>>);

impl Params {
    pub fn from_values(values: Vec<Result<Value, rusqlite::Error>>) -> Self {
        Self(values)
    }

    fn into_values(self) -> Result<Vec<Value>, rusqlite::Error> {
        self.0.into_iter().collect()
    }
}

impl From<()> for Params {
    fn from((): ()) -> Self {
        Self::default()
    }
}

pub fn to_value<T: ToSql + ?Sized>(value: &T) -> Result<Value, rusqlite::Error> {
    match value.to_sql()? {
        ToSqlOutput::Borrowed(value) => Value::try_from(value).map_err(Into::into),
        ToSqlOutput::Owned(value) => Ok(value),
        _ => unreachable!("unsupported SQLite parameter output"),
    }
}

#[macro_export]
macro_rules! sqlite_params {
    ($($value:expr),* $(,)?) => {
        $crate::db::sqlite::Params::from_values(vec![
            $($crate::db::sqlite::to_value(&$value)),*
        ])
    };
}

pub use crate::sqlite_params as params;

pub struct Connection {
    inner: Arc<Mutex<rusqlite::Connection>>,
}

impl Connection {
    pub async fn open(path: PathBuf) -> Result<Self, Error> {
        let connection = tokio::task::spawn_blocking(move || {
            let connection = rusqlite::Connection::open(path)?;
            connection.busy_timeout(Duration::from_secs(30))?;
            connection.pragma_update(None, "foreign_keys", "ON")?;
            Ok::<_, rusqlite::Error>(connection)
        })
        .await
        .map_err(Error::BlockingTask)??;

        Ok(Self {
            inner: Arc::new(Mutex::new(connection)),
        })
    }

    pub async fn execute<P>(&self, sql: &str, params: P) -> Result<usize, Error>
    where
        P: Into<Params> + Send + 'static,
    {
        let sql = sql.to_owned();
        let params = params.into().into_values().map_err(Error::Sqlite)?;
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let connection = inner.lock().map_err(|_| Error::ConnectionPoisoned)?;
            connection
                .execute(&sql, params_from_iter(params.iter()))
                .map_err(Error::Sqlite)
        })
        .await
        .map_err(Error::BlockingTask)?
    }

    pub async fn query<P>(&self, sql: &str, params: P) -> Result<Rows, Error>
    where
        P: Into<Params> + Send + 'static,
    {
        let sql = sql.to_owned();
        let params = params.into().into_values().map_err(Error::Sqlite)?;
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let connection = inner.lock().map_err(|_| Error::ConnectionPoisoned)?;
            let mut statement = connection.prepare(&sql)?;
            let columns = statement
                .column_names()
                .into_iter()
                .map(str::to_owned)
                .collect();
            let column_count = statement.column_count();
            let mut cursor = statement.query(params_from_iter(params.iter()))?;
            let mut materialized_rows = Vec::new();

            while let Some(row) = cursor.next()? {
                let mut values = Vec::with_capacity(column_count);
                for index in 0..column_count {
                    values
                        .push(Value::try_from(row.get_ref(index)?).map_err(rusqlite::Error::from)?);
                }
                materialized_rows.push(Row { values });
            }

            Ok(Rows {
                columns,
                rows: materialized_rows.into_iter(),
            })
        })
        .await
        .map_err(Error::BlockingTask)?
    }
}

pub struct Rows {
    columns: Vec<String>,
    rows: std::vec::IntoIter<Row>,
}

impl Rows {
    pub fn column_names(&self) -> Vec<String> {
        self.columns.clone()
    }

    pub async fn next(&mut self) -> Result<Option<Row>, Error> {
        Ok(self.rows.next())
    }
}

pub struct Row {
    values: Vec<Value>,
}

impl Row {
    pub fn get<T: FromSql>(&self, index: usize) -> Result<T, Error> {
        let value = self
            .values
            .get(index)
            .ok_or(rusqlite::Error::InvalidColumnIndex(index))?;
        T::column_result(ValueRef::from(value)).map_err(|error| Error::Sqlite(error.into()))
    }

    pub fn get_value(&self, index: usize) -> Result<Value, Error> {
        self.values
            .get(index)
            .cloned()
            .ok_or_else(|| Error::Sqlite(rusqlite::Error::InvalidColumnIndex(index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path() -> PathBuf {
        std::env::temp_dir().join(format!("twig-rusqlite-adapter-{}.db", uuid::Uuid::new_v4()))
    }

    #[tokio::test]
    async fn adapter_binds_supported_sqlite_values_and_materializes_rows() {
        let path = test_path();
        let connection = Connection::open(path.clone()).await.expect("open database");
        connection
            .execute(
                "CREATE TABLE values_fixture (n INTEGER, real_value REAL, text_value TEXT, blob_value BLOB, nullable TEXT)",
                (),
            )
            .await
            .expect("create fixture");
        connection
            .execute(
                "INSERT INTO values_fixture VALUES (?1, ?2, ?3, ?4, ?5)",
                crate::db::sqlite::params![
                    7_i64,
                    1.5_f64,
                    "hello",
                    vec![0_u8, 255],
                    None::<String>
                ],
            )
            .await
            .expect("insert values");

        let mut rows = connection
            .query("SELECT * FROM values_fixture", ())
            .await
            .expect("query values");
        assert_eq!(
            rows.column_names(),
            ["n", "real_value", "text_value", "blob_value", "nullable"]
        );
        let row = rows.next().await.expect("read row").expect("one row");
        assert_eq!(row.get::<i64>(0).expect("integer"), 7);
        assert_eq!(row.get::<f64>(1).expect("real"), 1.5);
        assert_eq!(row.get::<String>(2).expect("text"), "hello");
        assert_eq!(row.get_value(3).expect("blob"), Value::Blob(vec![0, 255]));
        assert_eq!(row.get::<Option<String>>(4).expect("null"), None);
        assert!(rows.next().await.expect("end of rows").is_none());

        drop(connection);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn every_connection_enforces_foreign_keys() {
        let path = test_path();
        let connection = Connection::open(path.clone()).await.expect("open database");
        let mut timeout = connection
            .query("PRAGMA busy_timeout", ())
            .await
            .expect("read busy timeout");
        let timeout_row = timeout
            .next()
            .await
            .expect("read timeout row")
            .expect("row exists");
        assert_eq!(timeout_row.get::<i64>(0).expect("timeout value"), 30_000);
        connection
            .execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", ())
            .await
            .expect("create parent");
        connection
            .execute(
                "CREATE TABLE child (parent_id INTEGER REFERENCES parent(id))",
                (),
            )
            .await
            .expect("create child");
        assert!(
            connection
                .execute("INSERT INTO child VALUES (1)", ())
                .await
                .is_err()
        );

        drop(connection);
        let _ = std::fs::remove_file(path);
    }
}
