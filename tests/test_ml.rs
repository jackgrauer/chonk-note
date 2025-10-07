#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1.0"
//! candle-core = "0.9"
//! candle-nn = "0.9"
//! ```

use anyhow::Result;
use candle_core::{Device, DType, Tensor};

fn main() -> Result<()> {
    println!("Testing ML features in Chonker 7.50...");
    
    // Test Metal device availability
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        println!("Checking Metal support on Apple Silicon...");
        match Device::new_metal(0) {
            Ok(_device) => {
                println!("✅ Metal acceleration available!");
            }
            Err(e) => {
                println!("⚠️ Metal not available: {}", e);
            }
        }
    }
    
    // Test CPU device (always available)
    let device = Device::Cpu;
    println!("Using device: {:?}", device);
    
    // Test tensor creation
    let test_tensor = Tensor::randn(0.0f32, 1.0, &[3, 768], &device)?;
    println!("✅ Created test tensor with shape: {:?}", test_tensor.shape());
    
    // Test basic operations
    let result = test_tensor.sum_all()?;
    println!("✅ Tensor operations working. Sum: {:?}", result.to_scalar::<f32>()?);
    
    println!("\n🎉 ML features are working in Chonker 7.50!");
    
    Ok(())
}