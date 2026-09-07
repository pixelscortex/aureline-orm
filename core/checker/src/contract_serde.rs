//! Logical contract representation for table-check Findings.
//!
//! Findings retain source spans and compilation-local owner IDs for semantic
//! consumers. Contract tests compare the stable logical problem shape, so this
//! adapter intentionally projects those location and identity details out of
//! the S-expression while the Rust tests inspect them directly when needed.

use serde::{Serialize, Serializer};

use crate::finding::Finding;

impl Serialize for Finding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::DuplicateTable { name, .. } => {
                ContractFinding::DuplicateTable { name }.serialize(serializer)
            }
            Self::DuplicateField { name, .. } => {
                ContractFinding::DuplicateField { name }.serialize(serializer)
            }
        }
    }
}

#[derive(Serialize)]
enum ContractFinding<'finding> {
    DuplicateTable { name: &'finding str },
    DuplicateField { name: &'finding str },
}
