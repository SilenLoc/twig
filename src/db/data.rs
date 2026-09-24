use crate::db::Database;

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

    pub async fn read_table_page(
        &self,
        table_name: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Option<TablePage>, String> {
        let table_names = self.list_table_names().await?;
        if !table_names.iter().any(|name| name == table_name) {
            return Ok(None);
        }

        let escaped_name = table_name.replace('"', "\"\"");
        let query = format!(
            "SELECT * FROM \"{escaped_name}\" LIMIT {} OFFSET {}",
            limit.min(100),
            offset.min(1_000_000)
        );
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
        let db_path = format!("/tmp/test_fig_database_view_{}.db", uuid::Uuid::new_v4());
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
}
