//! Console device driver

use crate::{Result, Error};
use crate::drivers::{DeviceType, DeviceOps, DeviceInfo, DeviceStatus};

/// Initialize console driver
pub fn init() -> Result<()> {
    crate::info!("Initializing console driver");
    Ok(())
}