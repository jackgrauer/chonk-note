/// Bridge to Apple's CoreML framework for ANE acceleration
/// This module provides FFI bindings and safe wrappers for CoreML

use anyhow::{Result, anyhow};
use candle_core::{Tensor, Device, DType};
use std::ffi::{CString, c_void};
use std::path::Path;

#[cfg(feature = "coreml")]
use objc::{msg_send, sel, sel_impl};
#[cfg(feature = "coreml")]
use objc::runtime::{Object, Class};
#[cfg(feature = "coreml")]
use core_foundation::{
    array::CFArray,
    base::{CFRelease, TCFType},
    dictionary::CFDictionary,
    string::CFString,
};

/// Configuration for CoreML model
#[derive(Debug, Clone)]
pub struct CoreMLConfig {
    pub input_names: Vec<String>,
    pub output_name: String,
    pub input_shapes: Vec<(usize, usize)>, // Support for 2D shapes, extend as needed
}

/// CoreML model wrapper
pub struct CoreMLModel {
    #[cfg(feature = "coreml")]
    model: *mut Object,
    config: CoreMLConfig,
    device: Device,
}

#[cfg(feature = "coreml")]
impl CoreMLModel {
    /// Load a compiled CoreML model from disk
    pub fn load(path: &str, config: CoreMLConfig) -> Result<Self> {
        unsafe {
            // Load MLModel class
            let cls = Class::get("MLModel").ok_or_else(|| anyhow!("MLModel class not found"))?;
            
            // Create URL from path
            let url_cls = Class::get("NSURL").ok_or_else(|| anyhow!("NSURL class not found"))?;
            let path_str = CString::new(path)?;
            let url: *mut Object = msg_send![url_cls, fileURLWithPath:path_str.as_ptr()];
            
            // Compile model if needed
            let compile_url = Self::compile_model_if_needed(url)?;
            
            // Load compiled model
            let error: *mut Object = std::ptr::null_mut();
            let model: *mut Object = msg_send![cls, modelWithContentsOfURL:compile_url error:&error];
            
            if !error.is_null() {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let desc_str = Self::nsstring_to_string(desc);
                return Err(anyhow!("Failed to load CoreML model: {}", desc_str));
            }
            
            if model.is_null() {
                return Err(anyhow!("Failed to load CoreML model: null pointer"));
            }
            
            Ok(Self {
                model,
                config,
                device: Device::new_metal(0).unwrap_or(Device::Cpu),
            })
        }
    }
    
    /// Forward pass through the model
    pub fn forward(&self, inputs: &[&Tensor]) -> Result<Tensor> {
        unsafe {
            // Create MLFeatureProvider for inputs
            let feature_provider = self.create_feature_provider(inputs)?;
            
            // Create prediction options
            let options_cls = Class::get("MLPredictionOptions").ok_or_else(|| anyhow!("MLPredictionOptions not found"))?;
            let options: *mut Object = msg_send![options_cls, new];
            
            // Run prediction
            let error: *mut Object = std::ptr::null_mut();
            let output: *mut Object = msg_send![self.model, 
                predictionFromFeatures:feature_provider 
                options:options 
                error:&error
            ];
            
            if !error.is_null() {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let desc_str = Self::nsstring_to_string(desc);
                return Err(anyhow!("Prediction failed: {}", desc_str));
            }
            
            // Extract output tensor
            self.extract_output_tensor(output)
        }
    }
    
    fn compile_model_if_needed(url: *mut Object) -> Result<*mut Object> {
        unsafe {
            // Check if already compiled (.mlmodelc extension)
            let path: *mut Object = msg_send![url, path];
            let path_str = Self::nsstring_to_string(path);
            
            if path_str.ends_with(".mlmodelc") {
                return Ok(url);
            }
            
            // Compile the model
            let compiler_cls = Class::get("MLCompiler").ok_or_else(|| anyhow!("MLCompiler not found"))?;
            let error: *mut Object = std::ptr::null_mut();
            let compiled_url: *mut Object = msg_send![compiler_cls,
                compileModelAtURL:url
                error:&error
            ];
            
            if !error.is_null() {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let desc_str = Self::nsstring_to_string(desc);
                return Err(anyhow!("Model compilation failed: {}", desc_str));
            }
            
            Ok(compiled_url)
        }
    }
    
    fn create_feature_provider(&self, inputs: &[&Tensor]) -> Result<*mut Object> {
        unsafe {
            // Create MLDictionaryFeatureProvider
            let provider_cls = Class::get("MLDictionaryFeatureProvider")
                .ok_or_else(|| anyhow!("MLDictionaryFeatureProvider not found"))?;
            
            let dict_cls = Class::get("NSMutableDictionary")
                .ok_or_else(|| anyhow!("NSMutableDictionary not found"))?;
            let dict: *mut Object = msg_send![dict_cls, new];
            
            // Add each input tensor
            for (i, (tensor, name)) in inputs.iter().zip(&self.config.input_names).enumerate() {
                let ml_array = self.tensor_to_mlarray(tensor)?;
                let key = Self::string_to_nsstring(name);
                let _: () = msg_send![dict, setObject:ml_array forKey:key];
            }
            
            // Create provider from dictionary
            let error: *mut Object = std::ptr::null_mut();
            let provider: *mut Object = msg_send![provider_cls,
                alloc,
                initWithDictionary:dict
                error:&error
            ];
            
            if !error.is_null() {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let desc_str = Self::nsstring_to_string(desc);
                return Err(anyhow!("Failed to create feature provider: {}", desc_str));
            }
            
            Ok(provider)
        }
    }
    
    fn tensor_to_mlarray(&self, tensor: &Tensor) -> Result<*mut Object> {
        unsafe {
            let array_cls = Class::get("MLMultiArray")
                .ok_or_else(|| anyhow!("MLMultiArray not found"))?;
            
            // Get tensor shape
            let shape = tensor.dims();
            let shape_array = self.shape_to_nsarray(&shape)?;
            
            // Determine data type
            let ml_data_type = match tensor.dtype() {
                DType::F32 => 0x10000 | 32, // MLMultiArrayDataTypeFloat32
                DType::F64 => 0x10000 | 64, // MLMultiArrayDataTypeFloat64
                DType::I64 => 0x20000 | 64, // MLMultiArrayDataTypeInt64
                _ => return Err(anyhow!("Unsupported tensor dtype for CoreML")),
            };
            
            // Create MLMultiArray
            let error: *mut Object = std::ptr::null_mut();
            let ml_array: *mut Object = msg_send![array_cls,
                alloc,
                initWithShape:shape_array
                dataType:ml_data_type
                error:&error
            ];
            
            if !error.is_null() {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let desc_str = Self::nsstring_to_string(desc);
                return Err(anyhow!("Failed to create MLMultiArray: {}", desc_str));
            }
            
            // Copy data
            let data_ptr: *mut c_void = msg_send![ml_array, dataPointer];
            match tensor.dtype() {
                DType::F32 => {
                    let tensor_data = tensor.to_vec1::<f32>()?;
                    std::ptr::copy_nonoverlapping(
                        tensor_data.as_ptr(),
                        data_ptr as *mut f32,
                        tensor_data.len()
                    );
                }
                DType::F64 => {
                    let tensor_data = tensor.to_vec1::<f64>()?;
                    std::ptr::copy_nonoverlapping(
                        tensor_data.as_ptr(),
                        data_ptr as *mut f64,
                        tensor_data.len()
                    );
                }
                _ => return Err(anyhow!("Unsupported dtype")),
            }
            
            Ok(ml_array)
        }
    }
    
    fn extract_output_tensor(&self, output: *mut Object) -> Result<Tensor> {
        unsafe {
            // Get output feature by name
            let output_name = Self::string_to_nsstring(&self.config.output_name);
            let feature: *mut Object = msg_send![output, featureValueForName:output_name];
            
            if feature.is_null() {
                return Err(anyhow!("Output feature '{}' not found", self.config.output_name));
            }
            
            // Get MLMultiArray from feature
            let ml_array: *mut Object = msg_send![feature, multiArrayValue];
            
            if ml_array.is_null() {
                return Err(anyhow!("Output is not a multi-array"));
            }
            
            // Extract shape
            let shape_obj: *mut Object = msg_send![ml_array, shape];
            let shape = self.nsarray_to_shape(shape_obj)?;
            
            // Extract data type and create tensor
            let data_type: i64 = msg_send![ml_array, dataType];
            let data_ptr: *mut c_void = msg_send![ml_array, dataPointer];
            let count: usize = msg_send![ml_array, count];
            
            match data_type {
                t if t == (0x10000 | 32) => { // Float32
                    let slice = std::slice::from_raw_parts(data_ptr as *const f32, count);
                    Tensor::from_vec(slice.to_vec(), shape.as_slice(), &self.device)
                }
                t if t == (0x10000 | 64) => { // Float64
                    let slice = std::slice::from_raw_parts(data_ptr as *const f64, count);
                    Tensor::from_vec(slice.to_vec(), shape.as_slice(), &self.device)
                }
                _ => Err(anyhow!("Unsupported output data type: {}", data_type)),
            }
        }
    }
    
    fn shape_to_nsarray(&self, shape: &[usize]) -> Result<*mut Object> {
        unsafe {
            let array_cls = Class::get("NSMutableArray")
                .ok_or_else(|| anyhow!("NSMutableArray not found"))?;
            let array: *mut Object = msg_send![array_cls, arrayWithCapacity:shape.len()];
            
            let number_cls = Class::get("NSNumber")
                .ok_or_else(|| anyhow!("NSNumber not found"))?;
            
            for &dim in shape {
                let num: *mut Object = msg_send![number_cls, numberWithUnsignedLong:dim];
                let _: () = msg_send![array, addObject:num];
            }
            
            Ok(array)
        }
    }
    
    fn nsarray_to_shape(&self, array: *mut Object) -> Result<Vec<usize>> {
        unsafe {
            let count: usize = msg_send![array, count];
            let mut shape = Vec::with_capacity(count);
            
            for i in 0..count {
                let num: *mut Object = msg_send![array, objectAtIndex:i];
                let val: usize = msg_send![num, unsignedLongValue];
                shape.push(val);
            }
            
            Ok(shape)
        }
    }
    
    fn string_to_nsstring(s: &str) -> *mut Object {
        unsafe {
            let string_cls = Class::get("NSString").expect("NSString class");
            let c_str = CString::new(s).expect("CString conversion");
            msg_send![string_cls, stringWithUTF8String:c_str.as_ptr()]
        }
    }
    
    fn nsstring_to_string(ns_str: *mut Object) -> String {
        unsafe {
            let c_str: *const i8 = msg_send![ns_str, UTF8String];
            let c_str = std::ffi::CStr::from_ptr(c_str);
            c_str.to_string_lossy().into_owned()
        }
    }
}

#[cfg(feature = "coreml")]
impl Drop for CoreMLModel {
    fn drop(&mut self) {
        unsafe {
            if !self.model.is_null() {
                let _: () = msg_send![self.model, release];
            }
        }
    }
}

// Stub implementation for non-macOS platforms
#[cfg(not(feature = "coreml"))]
impl CoreMLModel {
    pub fn load(_path: &str, _config: CoreMLConfig) -> Result<Self> {
        Err(anyhow!("CoreML is only available on macOS"))
    }
    
    pub fn forward(&self, _inputs: &[&Tensor]) -> Result<Tensor> {
        Err(anyhow!("CoreML is only available on macOS"))
    }
}