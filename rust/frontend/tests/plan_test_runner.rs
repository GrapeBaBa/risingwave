// Data-driven tests.

use anyhow::{anyhow, Result};
use frontend::binder::Binder;
use frontend::handler::{create_table, drop_table};
use frontend::planner::Planner;
use frontend::session::SessionImpl;
use frontend::test_utils::LocalFrontend;
use risingwave_common::array::RwError;
use risingwave_sqlparser::ast::{ObjectName, Statement};
use risingwave_sqlparser::parser::Parser;
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct TestCase {
    sql: String,
    plan: Option<String>,
    binder_error: Option<String>,
    optimizer_error: Option<String>,
}

impl TestCase {
    async fn run(&self) -> Result<()> {
        let frontend = LocalFrontend::new().await;
        let session = frontend.session();

        let statements = Parser::parse_sql(&self.sql).unwrap();
        for stmt in statements {
            match stmt.clone() {
                Statement::Query(_) | Statement::Insert { .. } => {
                    self.test_query(&stmt, session)?
                }
                Statement::CreateTable { name, columns, .. } => {
                    create_table::handle_create_table(session, name, columns).await?;
                }
                Statement::Drop(drop_statement) => {
                    let table_object_name = ObjectName(vec![drop_statement.name]);
                    drop_table::handle_drop_table(session, table_object_name).await?;
                }
                _ => return Err(anyhow!("Unsupported statement type")),
            }
        }
        Ok(())
    }

    fn test_query(&self, stmt: &Statement, session: &SessionImpl) -> Result<()> {
        let catalog = session
            .env()
            .catalog_mgr()
            .get_database_snapshot(session.database())
            .unwrap();
        let mut binder = Binder::new(catalog);

        let bound = check_err("binder", &self.binder_error, binder.bind(stmt.clone()))?;
        if bound.is_none() {
            return Ok(());
        }

        let plan = Planner::new().plan(bound.unwrap())?;
        let mut output = String::new();
        plan.explain(0, &mut output)?;
        let expected_plan = self.plan.as_ref().unwrap().clone();
        if expected_plan != output {
            Err(anyhow!(
                "Expected plan:\n{}\nActual plan:\n{}",
                expected_plan,
                output
            ))
        } else {
            Ok(())
        }
    }
}

fn check_err<T>(
    ctx: &str,
    expected_err: &Option<String>,
    actual_res: std::result::Result<T, RwError>,
) -> Result<Option<T>> {
    match (expected_err, actual_res) {
        (None, Ok(t)) => Ok(Some(t)),
        (None, Err(e)) => Err(anyhow!("unexpected {} error: {}", ctx, e)),
        (Some(e), Ok(_)) => Err(anyhow!("expected {} error: {}", ctx, e)),
        (Some(l), Err(r)) => {
            if *l == r.to_string() {
                Ok(None)
            } else {
                return Err(anyhow!(
                    "Expected {context} error: {}, Actual {context} error: {}",
                    l,
                    r,
                    context = ctx
                ));
            }
        }
    }
}

async fn run_test_file(_file_name: &str, file_content: &str) {
    let mut failed_num = 0;
    let cases: Vec<TestCase> = serde_yaml::from_str(file_content).unwrap();
    for c in cases {
        if let Err(e) = c.run().await {
            println!("\nTest case failed, the input SQL:\n  {}\n{}", c.sql, e);
            failed_num += 1;
        }
    }
    if failed_num > 0 {
        println!("\n");
        panic!("{} test cases failed", failed_num);
    }
}

// Traverses the 'testdata/' directory and runs all files.
#[tokio::test]
async fn run_all_test_files() {
    use walkdir::WalkDir;
    for entry in WalkDir::new("./tests/testdata/") {
        let entry = entry.unwrap();
        if !entry.path().is_file() {
            continue;
        }
        let file_content = std::fs::read_to_string(entry.path()).unwrap();
        run_test_file(entry.path().to_str().unwrap(), &file_content).await;
    }
}