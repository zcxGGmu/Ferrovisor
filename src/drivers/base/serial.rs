//! Serial port device driver

use crate::{Result, Error};
use crate::drivers::{DeviceType, DeviceOps, DeviceInfo, DeviceStatus};

/// Initialize serial driver
pub fn init() -> Result<()> {
    crate::info!("Initializing serial driver");
    Ok(())
}