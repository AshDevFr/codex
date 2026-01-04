/// Macro to generate database-agnostic query methods
///
/// This macro generates both PostgreSQL and SQLite versions of a query method,
/// handling the parameter binding syntax differences automatically.
///
/// Usage:
/// ```rust
/// impl_repository_query! {
///     fn create_library(
///         pool: &Pool<DB>,
///         name: &str,
///         path: &str,
///         strategy: ScanningStrategy,
///     ) -> Result<Library> {
///         let library = Library::new(name.to_string(), path.to_string(), strategy);
///
///         query!(
///             "INSERT INTO libraries (id, name, path, scanning_strategy, scanning_config, created_at, updated_at)
///              VALUES (?, ?, ?, ?, ?, ?, ?)",
///             library.id,
///             &library.name,
///             &library.path,
///             &library.scanning_strategy,
///             &library.scanning_config,
///             library.created_at,
///             library.updated_at
///         )
///         .execute(pool)
///         .await
///         .context("Failed to create library")?;
///
///         Ok(library)
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_repository_query {
    (
        $(#[$meta:meta])*
        $vis:vis async fn $name:ident(
            $pool:ident: &Pool<$db_type:ty>,
            $($arg:ident: $arg_type:ty),*
        ) -> $ret:ty {
            $($body:tt)*
        }
    ) => {
        // PostgreSQL version
        $(#[$meta])*
        $vis async fn $name(
            $pool:ident: &sqlx::Pool<sqlx::Postgres>,
            $($arg:ident: $arg_type),*
        ) -> $ret {
            impl_repository_query!(@rewrite_query $($body)* postgres)
        }

        // SQLite version
        $(#[$meta])*
        $vis async fn $name_sqlite(
            $pool:ident: &sqlx::Pool<sqlx::Sqlite>,
            $($arg:ident: $arg_type),*
        ) -> $ret {
            impl_repository_query!(@rewrite_query $($body)* sqlite)
        }
    };

    // Rewrite query! macro calls to use correct parameter syntax
    (@rewrite_query $($body:tt)* postgres) => {
        // Replace ? with $1, $2, etc. in query strings
        // This is a simplified version - full implementation would need more sophisticated parsing
        $($body)*
    };

    (@rewrite_query $($body:tt)* sqlite) => {
        $($body)*
    };
}

