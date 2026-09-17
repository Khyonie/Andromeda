use serde::{Deserialize, Serialize};

/// A presence-only XML element, such as `<readonly/>`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Empty {}
