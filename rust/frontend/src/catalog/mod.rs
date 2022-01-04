#![allow(dead_code)]
use risingwave_common::array::RwError;
use risingwave_common::error::ErrorCode;
use thiserror::Error;

mod catalog_service;
mod column_catalog;
mod create_table_info;
mod database_catalog;
mod schema_catalog;
mod table_catalog;

pub(crate) type DatabaseId = u32;
pub(crate) type SchemaId = u32;
pub(crate) type TableId = u32;
pub(crate) type ColumnId = u32;

#[derive(Error, Debug)]
pub enum CatalogError {
    #[error("{0} not found: {1}")]
    NotFound(&'static str, String),
    #[error("duplicated {0}: {1}")]
    Duplicated(&'static str, String),
}

impl From<CatalogError> for RwError {
    fn from(e: CatalogError) -> Self {
        ErrorCode::CatalogError(Box::new(e)).into()
    }
}