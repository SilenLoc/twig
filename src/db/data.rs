use crate::db::Database;

/// Upper bound on one page's rows. The database viewer renders every row of a
/// page into HTML, so a runaway page size should fail loudly here rather than
/// silently shrink or ship a megabyte of markup. A caller wanting larger pages
/// must raise this deliberately.
const MAX_PAGE_LIMIT: usize = 100;

#[derive(Debug)]
pub struct TablePage {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Database {
    pub async fn list_table_names(&self) -> Result<Vec<String>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut names = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            names.push(row.get(0).map_err(|e| e.to_string())?);
        }
        Ok(names)
    }

    /// Reads one page of rows, trusting the caller's `limit` and `offset`.
    /// An `offset` past the end of the table is not an error: it simply yields
    /// no rows, which is how the viewer's scroll chain terminates.
    pub async fn read_table_page(
        &self,
        table_name: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Option<TablePage>, String> {
        if limit > MAX_PAGE_LIMIT {
            return Err(format!(
                "page limit {limit} exceeds the maximum of {MAX_PAGE_LIMIT}"
            ));
        }

        let table_names = self.list_table_names().await?;
        if !table_names.iter().any(|name| name == table_name) {
            return Ok(None);
        }

        let escaped_name = table_name.replace('"', "\"\"");
        let query = format!("SELECT * FROM \"{escaped_name}\" LIMIT {limit} OFFSET {offset}");
        let mut rows = self
            .conn()
            .await?
            .query(query, ())
            .await
            .map_err(|e| e.to_string())?;
        let columns = rows.column_names();
        let mut values = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let mut value_row = Vec::with_capacity(columns.len());
            for index in 0..columns.len() {
                let value = row.get_value(index).map_err(|e| e.to_string())?;
                value_row.push(match value {
                    turso::Value::Null => "NULL".to_string(),
                    turso::Value::Integer(value) => value.to_string(),
                    turso::Value::Real(value) => value.to_string(),
                    turso::Value::Text(value) => value,
                    turso::Value::Blob(value) => format!("0x{}", hex::encode(value)),
                });
            }
            values.push(value_row);
        }

        Ok(Some(TablePage {
            columns,
            rows: values,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn table_reader_lists_tables_and_pages_values() {
        let db_path = format!("/tmp/test_twig_database_view_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");
        db.conn()
            .await
            .expect("database connection")
            .execute(
                "CREATE TABLE viewer_fixture (id INTEGER, value TEXT, payload BLOB)",
                (),
            )
            .await
            .expect("create fixture table");
        db.conn()
            .await
            .expect("database connection")
            .execute(
                "INSERT INTO viewer_fixture VALUES (1, 'hello', x'0102')",
                (),
            )
            .await
            .expect("insert fixture row");

        let tables = db.list_table_names().await.expect("list table names");
        assert!(tables.iter().any(|table| table == "viewer_fixture"));
        let page = db
            .read_table_page("viewer_fixture", 50, 0)
            .await
            .expect("read fixture table")
            .expect("fixture table exists");
        assert_eq!(page.columns, ["id", "value", "payload"]);
        assert_eq!(page.rows, [["1", "hello", "0x0102"]]);
        assert!(
            db.read_table_page("not_a_table", 50, 0)
                .await
                .expect("invalid table query")
                .is_none()
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn table_reader_rejects_an_oversized_page_loudly() {
        let db_path = format!("/tmp/test_twig_data_limit_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");

        let error = db
            .read_table_page("users", 101, 0)
            .await
            .expect_err("a limit over the cap must fail, not clamp silently");
        assert!(error.contains("exceeds the maximum of 100"), "{error}");
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn table_reader_returns_an_empty_page_past_the_end_of_the_table() {
        let db_path = format!("/tmp/test_twig_data_offset_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");
        db.conn()
            .await
            .expect("database connection")
            .execute("CREATE TABLE offset_fixture (id INTEGER)", ())
            .await
            .expect("create fixture table");
        db.conn()
            .await
            .expect("database connection")
            .execute("INSERT INTO offset_fixture VALUES (1)", ())
            .await
            .expect("insert fixture row");

        // An offset past the end is not clamped back into range; it yields an
        // empty page, which is how the viewer's scroll chain terminates.
        let page = db
            .read_table_page("offset_fixture", 50, 50)
            .await
            .expect("read past the end")
            .expect("fixture table exists");
        assert!(page.rows.is_empty(), "{page:?}");

        let page = db
            .read_table_page("offset_fixture", 50, 1_000_000)
            .await
            .expect("read far past the end")
            .expect("fixture table exists");
        assert!(page.rows.is_empty(), "{page:?}");

        let _ = std::fs::remove_file(db_path);
    }
}
