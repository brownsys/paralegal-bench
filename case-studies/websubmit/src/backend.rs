use mysql::prelude::*;
use mysql::Opts;
pub use mysql::Value;
use mysql::*;
use std::collections::HashMap;

pub struct MySqlBackend {
    pub handle: mysql::Conn,
    pub log: slog::Logger,
    _schema: String,
    prep_stmts: HashMap<String, mysql::Statement>,
}

impl MySqlBackend {
    pub fn new(dbname: &str, log: Option<slog::Logger>, prime: bool) -> Result<Self> {
        let log = match log {
            None => slog::Logger::root(slog::Discard, o!()),
            Some(l) => l,
        };

        let schema = std::fs::read_to_string("src/schema.sql")?;

        debug!(
            log,
            "Connecting to MySql DB and initializing schema {}...", dbname
        );
        let mut db = mysql::Conn::new(
            Opts::from_url(&format!("mysql://root:password@127.0.0.1/{}", dbname)).unwrap(),
        )
        .unwrap();
        assert_eq!(db.ping(), true);

        if prime {
            db.query_drop(format!("DROP DATABASE IF EXISTS {};", dbname))
                .unwrap();
            db.query_drop(format!("CREATE DATABASE {};", dbname))
                .unwrap();
            // reconnect
            db = mysql::Conn::new(
                Opts::from_url(&format!("mysql://root:password@127.0.0.1/{}", dbname)).unwrap(),
            )
            .unwrap();
            for line in schema.lines() {
                if line.starts_with("--") || line.is_empty() {
                    continue;
                }
                db.query_drop(line).unwrap();
            }
        }

        Ok(MySqlBackend {
            handle: db,
            log: log,
            _schema: schema.to_owned(),
            prep_stmts: HashMap::new(),
        })
    }

    #[paralegal::marker(source)]
    #[cfg_attr(feature = "v-ann-lib", paralegal::marker(from_storage, return))]
    pub fn prep_exec(&mut self, sql: &str, params: Vec<Value>) -> Vec<Vec<Value>> {
        if !self.prep_stmts.contains_key(sql) {
            let stmt = self
                .handle
                .prep(sql)
                .expect(&format!("failed to prepare statement \'{}\'", sql));
            self.prep_stmts.insert(sql.to_owned(), stmt);
        }
        let res = self
            .handle
            .exec_iter(self.prep_stmts[sql].clone(), params)
            .expect(&format!("query \'{}\' failed", sql));
        let mut rows = vec![];
        for row in res {
            let rowvals = row.unwrap().unwrap();
            let vals: Vec<Value> = rowvals.iter().map(|v| v.clone().into()).collect();
            rows.push(vals);
        }
        debug!(self.log, "executed query {}, got {} rows", sql, rows.len());
        return rows;
    }

    fn do_insert(&mut self, table: &str, vals: Vec<Value>, replace: bool) {
        let op = if replace { "REPLACE" } else { "INSERT" };
        let q = format!(
            "{} INTO {} VALUES ({})",
            op,
            table,
            vals.iter().map(|_| "?").collect::<Vec<&str>>().join(",")
        );
        debug!(self.log, "executed insert query {} for row {:?}", q, vals);
        self.handle
            .exec_drop(q.clone(), vals)
            .expect(&format!("failed to insert into {}, query {}!", table, q));
    }

    #[paralegal::marker{ stores, arguments = [2] }]
    #[paralegal::marker{ scopes_store, arguments = [2] }]
    pub fn insert(&mut self, table: &str, vals: Vec<Value>) {
        self.do_insert(table, vals, false);
    }

    #[cfg_attr(not(feature = "v-ann-strict"), paralegal::marker{ scopes_store, arguments = [2] })]
    #[paralegal::marker{ stores, arguments = [2] }]
    pub fn replace(&mut self, table: &str, vals: Vec<Value>) {
        self.do_insert(table, vals, true);
    }

    #[paralegal::marker(deletes, arguments = [2])]
    pub fn delete(&mut self, table: &str, criteria: &[(&str, Value)]) {
        let (where_parts, vals): (Vec<_>, Vec<_>) = criteria
            .iter()
            .map(|(id, v)| (format!("{id} = ?"), v))
            .unzip();
        let where_ = where_parts.join(" AND ");
        self.handle
            .exec_drop(&format!("DELETE FROM {table} WHERE {where_}"), vals)
            .unwrap();
    }
}
