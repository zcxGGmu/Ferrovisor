//! Timer device driver

use crate::{Result, Error};

/// Initialize timer driver
pub fn init() -> Result<()> {
    crate::info!("Initializing timer driver");
    Ok(())
}
