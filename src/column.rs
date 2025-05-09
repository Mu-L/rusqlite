use std::str;

use crate::{Error, Result, Statement};

/// Information about a column of a SQLite query.
#[cfg(feature = "column_decltype")]
#[derive(Debug)]
pub struct Column<'stmt> {
    name: &'stmt str,
    decl_type: Option<&'stmt str>,
}

#[cfg(feature = "column_decltype")]
impl Column<'_> {
    /// Returns the name of the column.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name
    }

    /// Returns the type of the column (`None` for expression).
    #[inline]
    #[must_use]
    pub fn decl_type(&self) -> Option<&str> {
        self.decl_type
    }
}

/// Metadata about the origin of a column of a SQLite query
#[cfg(feature = "column_metadata")]
#[derive(Debug)]
pub struct ColumnMetadata<'stmt> {
    name: &'stmt str,
    database_name: Option<&'stmt str>,
    table_name: Option<&'stmt str>,
    origin_name: Option<&'stmt str>,
}

#[cfg(feature = "column_metadata")]
impl ColumnMetadata<'_> {
    #[inline]
    #[must_use]
    /// Returns the name of the column in the query results
    pub fn name(&self) -> &str {
        self.name
    }

    #[inline]
    #[must_use]
    /// Returns the database name from which the column originates
    pub fn database_name(&self) -> Option<&str> {
        self.database_name
    }

    #[inline]
    #[must_use]
    /// Returns the table name from which the column originates
    pub fn table_name(&self) -> Option<&str> {
        self.table_name
    }

    #[inline]
    #[must_use]
    /// Returns the column name from which the column originates
    pub fn origin_name(&self) -> Option<&str> {
        self.origin_name
    }
}

impl Statement<'_> {
    /// Get all the column names in the result set of the prepared statement.
    ///
    /// If associated DB schema can be altered concurrently, you should make
    /// sure that current statement has already been stepped once before
    /// calling this method.
    pub fn column_names(&self) -> Vec<&str> {
        let n = self.column_count();
        let mut cols = Vec::with_capacity(n);
        for i in 0..n {
            let s = self.column_name_unwrap(i);
            cols.push(s);
        }
        cols
    }

    /// Return the number of columns in the result set returned by the prepared
    /// statement.
    ///
    /// If associated DB schema can be altered concurrently, you should make
    /// sure that current statement has already been stepped once before
    /// calling this method.
    #[inline]
    pub fn column_count(&self) -> usize {
        self.stmt.column_count()
    }

    /// Check that column name reference lifetime is limited:
    /// <https://www.sqlite.org/c3ref/column_name.html>
    /// > The returned string pointer is valid...
    ///
    /// `column_name` reference can become invalid if `stmt` is reprepared
    /// (because of schema change) when `query_row` is called. So we assert
    /// that a compilation error happens if this reference is kept alive:
    /// ```compile_fail
    /// use rusqlite::{Connection, Result};
    /// fn main() -> Result<()> {
    ///     let db = Connection::open_in_memory()?;
    ///     let mut stmt = db.prepare("SELECT 1 as x")?;
    ///     let column_name = stmt.column_name(0)?;
    ///     let x = stmt.query_row([], |r| r.get::<_, i64>(0))?; // E0502
    ///     assert_eq!(1, x);
    ///     assert_eq!("x", column_name);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub(super) fn column_name_unwrap(&self, col: usize) -> &str {
        // Just panic if the bounds are wrong for now, we never call this
        // without checking first.
        self.column_name(col).expect("Column out of bounds")
    }

    /// Returns the name assigned to a particular column in the result set
    /// returned by the prepared statement.
    ///
    /// If associated DB schema can be altered concurrently, you should make
    /// sure that current statement has already been stepped once before
    /// calling this method.
    ///
    /// ## Failure
    ///
    /// Returns an `Error::InvalidColumnIndex` if `idx` is outside the valid
    /// column range for this row.
    ///
    /// # Panics
    ///
    /// Panics when column name is not valid UTF-8.
    #[inline]
    pub fn column_name(&self, col: usize) -> Result<&str> {
        self.stmt
            .column_name(col)
            // clippy::or_fun_call (nightly) vs clippy::unnecessary-lazy-evaluations (stable)
            .ok_or(Error::InvalidColumnIndex(col))
            .map(|slice| {
                slice
                    .to_str()
                    .expect("Invalid UTF-8 sequence in column name")
            })
    }

    /// Returns the column index in the result set for a given column name.
    ///
    /// If there is no AS clause then the name of the column is unspecified and
    /// may change from one release of SQLite to the next.
    ///
    /// If associated DB schema can be altered concurrently, you should make
    /// sure that current statement has already been stepped once before
    /// calling this method.
    ///
    /// # Failure
    ///
    /// Will return an `Error::InvalidColumnName` when there is no column with
    /// the specified `name`.
    #[inline]
    pub fn column_index(&self, name: &str) -> Result<usize> {
        let bytes = name.as_bytes();
        let n = self.column_count();
        for i in 0..n {
            // Note: `column_name` is only fallible if `i` is out of bounds,
            // which we've already checked.
            if bytes.eq_ignore_ascii_case(self.stmt.column_name(i).unwrap().to_bytes()) {
                return Ok(i);
            }
        }
        Err(Error::InvalidColumnName(String::from(name)))
    }

    /// Returns a slice describing the columns of the result of the query.
    ///
    /// If associated DB schema can be altered concurrently, you should make
    /// sure that current statement has already been stepped once before
    /// calling this method.
    #[cfg(feature = "column_decltype")]
    pub fn columns(&self) -> Vec<Column> {
        let n = self.column_count();
        let mut cols = Vec::with_capacity(n);
        for i in 0..n {
            let name = self.column_name_unwrap(i);
            let slice = self.stmt.column_decltype(i);
            let decl_type = slice.map(|s| {
                s.to_str()
                    .expect("Invalid UTF-8 sequence in column declaration")
            });
            cols.push(Column { name, decl_type });
        }
        cols
    }

    /// Returns the names of the database, table, and row from which
    /// each column of this query's results originate.
    ///
    /// Computed or otherwise derived columns will have None values for these fields.
    #[cfg(feature = "column_metadata")]
    pub fn columns_with_metadata(&self) -> Vec<ColumnMetadata> {
        let n = self.column_count();
        let mut col_mets = Vec::with_capacity(n);
        for i in 0..n {
            let name = self.column_name_unwrap(i);
            let db_slice = self.stmt.column_database_name(i);
            let tbl_slice = self.stmt.column_table_name(i);
            let origin_slice = self.stmt.column_origin_name(i);
            col_mets.push(ColumnMetadata {
                name,
                database_name: db_slice.map(|s| {
                    s.to_str()
                        .expect("Invalid UTF-8 sequence in column db name")
                }),
                table_name: tbl_slice.map(|s| {
                    s.to_str()
                        .expect("Invalid UTF-8 sequence in column table name")
                }),
                origin_name: origin_slice.map(|s| {
                    s.to_str()
                        .expect("Invalid UTF-8 sequence in column origin name")
                }),
            })
        }
        col_mets
    }
}

#[cfg(test)]
mod test {
    use crate::{Connection, Result};

    #[test]
    #[cfg(feature = "column_decltype")]
    fn test_columns() -> Result<()> {
        use super::Column;

        let db = Connection::open_in_memory()?;
        let query = db.prepare("SELECT * FROM sqlite_master")?;
        let columns = query.columns();
        let column_names: Vec<&str> = columns.iter().map(Column::name).collect();
        assert_eq!(
            column_names.as_slice(),
            &["type", "name", "tbl_name", "rootpage", "sql"]
        );
        let column_types: Vec<Option<String>> = columns
            .iter()
            .map(|col| col.decl_type().map(str::to_lowercase))
            .collect();
        assert_eq!(
            &column_types[..3],
            &[
                Some("text".to_owned()),
                Some("text".to_owned()),
                Some("text".to_owned()),
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg(feature = "column_metadata")]
    fn test_columns_with_metadata() -> Result<()> {
        let db = Connection::open_in_memory()?;
        let query = db.prepare("SELECT *, 1 FROM sqlite_master")?;

        let col_mets = query.columns_with_metadata();

        assert_eq!(col_mets.len(), 6);

        for col in col_mets.iter().take(5) {
            assert_eq!(&col.database_name(), &Some("main"));
            assert_eq!(&col.table_name(), &Some("sqlite_master"));
        }

        assert!(col_mets[5].database_name().is_none());
        assert!(col_mets[5].table_name().is_none());
        assert!(col_mets[5].origin_name().is_none());

        let col_origins: Vec<Option<&str>> = col_mets.iter().map(|col| col.origin_name()).collect();

        assert_eq!(
            &col_origins[..5],
            &[
                Some("type"),
                Some("name"),
                Some("tbl_name"),
                Some("rootpage"),
                Some("sql"),
            ]
        );

        Ok(())
    }

    #[test]
    fn test_column_name_in_error() -> Result<()> {
        use crate::{types::Type, Error};
        let db = Connection::open_in_memory()?;
        db.execute_batch(
            "BEGIN;
             CREATE TABLE foo(x INTEGER, y TEXT);
             INSERT INTO foo VALUES(4, NULL);
             END;",
        )?;
        let mut stmt = db.prepare("SELECT x as renamed, y FROM foo")?;
        let mut rows = stmt.query([])?;
        let row = rows.next()?.unwrap();
        match row.get::<_, String>(0).unwrap_err() {
            Error::InvalidColumnType(idx, name, ty) => {
                assert_eq!(idx, 0);
                assert_eq!(name, "renamed");
                assert_eq!(ty, Type::Integer);
            }
            e => {
                panic!("Unexpected error type: {e:?}");
            }
        }
        match row.get::<_, String>("y").unwrap_err() {
            Error::InvalidColumnType(idx, name, ty) => {
                assert_eq!(idx, 1);
                assert_eq!(name, "y");
                assert_eq!(ty, Type::Null);
            }
            e => {
                panic!("Unexpected error type: {e:?}");
            }
        }
        Ok(())
    }

    /// `column_name` reference should stay valid until `stmt` is reprepared (or
    /// reset) even if DB schema is altered (SQLite documentation is
    /// ambiguous here because it says reference "is valid until (...) the next
    /// call to `sqlite3_column_name()` or `sqlite3_column_name16()` on the same
    /// column.". We assume that reference is valid if only
    /// `sqlite3_column_name()` is used):
    #[test]
    #[cfg(feature = "modern_sqlite")]
    fn test_column_name_reference() -> Result<()> {
        let db = Connection::open_in_memory()?;
        db.execute_batch("CREATE TABLE y (x);")?;
        let stmt = db.prepare("SELECT x FROM y;")?;
        let column_name = stmt.column_name(0)?;
        assert_eq!("x", column_name);
        db.execute_batch("ALTER TABLE y RENAME COLUMN x TO z;")?;
        // column name is not refreshed until statement is re-prepared
        let same_column_name = stmt.column_name(0)?;
        assert_eq!(same_column_name, column_name);
        Ok(())
    }
}
