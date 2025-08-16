use anyhow::Result;
use candle_core::{Tensor, Device, DType};
use crate::content_extractor::{CharacterData, PageData};

/// Chunking configuration for long documents
pub struct ChunkConfig {
    pub max_tokens: usize,      // Maximum tokens per chunk (512 optimal for ANE)
    pub overlap_tokens: usize,  // Overlap between chunks (64 default)
    pub stride: usize,          // Sliding window stride
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            overlap_tokens: 64,
            stride: 448, // max_tokens - overlap_tokens
        }
    }
}

/// Document chunk for processing
#[derive(Debug, Clone)]
pub struct DocumentChunk {
    pub characters: Vec<CharacterData>,
    pub start_idx: usize,
    pub end_idx: usize,
    pub chunk_id: usize,
    pub total_chunks: usize,
}

/// Chunked document processor for handling long documents
pub struct ChunkedProcessor {
    config: ChunkConfig,
    device: Device,
}

impl ChunkedProcessor {
    pub fn new(config: ChunkConfig) -> Result<Self> {
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);
        Ok(Self { config, device })
    }
    
    /// Split document into overlapping chunks
    pub fn create_chunks(&self, page_data: &PageData) -> Vec<DocumentChunk> {
        let mut chunks = Vec::new();
        let total_chars = page_data.characters.len();
        
        if total_chars <= self.config.max_tokens {
            // Single chunk for short documents
            chunks.push(DocumentChunk {
                characters: page_data.characters.clone(),
                start_idx: 0,
                end_idx: total_chars,
                chunk_id: 0,
                total_chunks: 1,
            });
            return chunks;
        }
        
        // Create overlapping chunks
        let mut start = 0;
        let mut chunk_id = 0;
        
        while start < total_chars {
            let end = (start + self.config.max_tokens).min(total_chars);
            
            chunks.push(DocumentChunk {
                characters: page_data.characters[start..end].to_vec(),
                start_idx: start,
                end_idx: end,
                chunk_id,
                total_chunks: 0, // Will be updated
            });
            
            chunk_id += 1;
            start += self.config.stride;
            
            // Ensure we don't miss the end
            if start < total_chars && start + self.config.max_tokens >= total_chars {
                start = total_chars.saturating_sub(self.config.max_tokens);
            }
        }
        
        // Update total chunks count
        let total_chunks = chunks.len();
        for chunk in &mut chunks {
            chunk.total_chunks = total_chunks;
        }
        
        chunks
    }
    
    /// Merge outputs from overlapping chunks
    pub fn merge_chunk_outputs(&self, outputs: Vec<Tensor>) -> Result<Tensor> {
        if outputs.is_empty() {
            return Err(anyhow::anyhow!("No outputs to merge"));
        }
        
        if outputs.len() == 1 {
            return Ok(outputs.into_iter().next().unwrap());
        }
        
        // Get dimensions
        let (batch_size, _, hidden_size) = outputs[0].dims3()?;
        
        // Calculate total sequence length (accounting for overlap)
        let mut total_seq_len = 0;
        for (i, output) in outputs.iter().enumerate() {
            let seq_len = output.dim(1)?;
            if i == 0 {
                total_seq_len += seq_len;
            } else {
                // Subtract overlap
                total_seq_len += seq_len.saturating_sub(self.config.overlap_tokens);
            }
        }
        
        // Create merged tensor
        let mut merged_data = Vec::with_capacity(total_seq_len * hidden_size);
        
        for (i, output) in outputs.iter().enumerate() {
            let output_data = output.to_vec3::<f32>()?;
            
            if i == 0 {
                // First chunk - use all tokens
                for token_hidden in &output_data[0] {
                    merged_data.extend_from_slice(token_hidden);
                }
            } else {
                // Subsequent chunks - skip overlap tokens
                for token_hidden in output_data[0].iter().skip(self.config.overlap_tokens) {
                    merged_data.extend_from_slice(token_hidden);
                }
            }
        }
        
        Ok(Tensor::from_vec(
            merged_data,
            &[batch_size, total_seq_len, hidden_size],
            &self.device
        )?)
    }
    
    /// Apply attention mask for overlapping regions
    pub fn create_attention_mask(&self, chunk: &DocumentChunk) -> Result<Tensor> {
        let seq_len = chunk.characters.len();
        let mut mask = vec![1.0f32; seq_len * seq_len];
        
        // If not first chunk, mask attention to overlap region from previous chunk
        if chunk.chunk_id > 0 {
            for i in 0..self.config.overlap_tokens {
                for j in self.config.overlap_tokens..seq_len {
                    // Prevent attending from non-overlap to overlap
                    mask[j * seq_len + i] = 0.0;
                }
            }
        }
        
        // If not last chunk, mask attention to overlap region for next chunk
        if chunk.chunk_id < chunk.total_chunks - 1 {
            let start = seq_len - self.config.overlap_tokens;
            for i in start..seq_len {
                for j in 0..start {
                    // Prevent attending from non-overlap to overlap
                    mask[j * seq_len + i] = 0.0;
                }
            }
        }
        
        Ok(Tensor::from_vec(mask, &[1, seq_len, seq_len], &self.device)?)
    }
}

/// Prepare text tensor from character data
pub fn prepare_text_tensor(
    chars: &[CharacterData],
    tokenizer: &tokenizers::Tokenizer,
    device: &Device,
) -> Result<Tensor> {
    let text: String = chars.iter().map(|c| c.unicode).collect();
    let encoding = tokenizer.encode(text, false)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
    
    Ok(Tensor::from_vec(
        encoding.get_ids().to_vec(),
        &[1, encoding.get_ids().len()],
        device
    )?)
}

/// Prepare bbox tensor with normalization
pub fn prepare_bbox_tensor(
    chars: &[CharacterData],
    page_width: f32,
    page_height: f32,
    device: &Device,
) -> Result<Tensor> {
    let mut bboxes = Vec::new();
    
    for ch in chars {
        // Normalize to 0-1000 range as per LayoutLM convention
        bboxes.push(((ch.x * 1000.0 / page_width) as i32).clamp(0, 1000) as f32);
        bboxes.push(((ch.y * 1000.0 / page_height) as i32).clamp(0, 1000) as f32);
        bboxes.push((((ch.x + ch.width) * 1000.0 / page_width) as i32).clamp(0, 1000) as f32);
        bboxes.push((((ch.y + ch.height) * 1000.0 / page_height) as i32).clamp(0, 1000) as f32);
    }
    
    Ok(Tensor::from_vec(
        bboxes,
        &[1, chars.len(), 4],
        device
    )?)
}

/// Prepare visual patches from PDF page image
pub fn prepare_visual_patches(
    image: &image::DynamicImage,
    patch_size: usize,
    device: &Device,
) -> Result<Tensor> {
    // Resize to standard ViT input size (224x224 for LayoutLMv3)
    let target_size = 224;
    let resized = image.resize_exact(
        target_size as u32,
        target_size as u32,
        image::imageops::FilterType::Lanczos3
    );
    
    let rgb = resized.to_rgb8();
    let mut patches = Vec::new();
    
    // ImageNet normalization constants
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];
    
    // Extract patches
    let num_patches = target_size / patch_size;
    
    for py in 0..num_patches {
        for px in 0..num_patches {
            let mut patch_data = Vec::new();
            
            for y in 0..patch_size {
                for x in 0..patch_size {
                    let pixel_x = px * patch_size + x;
                    let pixel_y = py * patch_size + y;
                    let pixel = rgb.get_pixel(pixel_x as u32, pixel_y as u32);
                    
                    // Normalize each channel
                    for (i, &val) in pixel.0.iter().enumerate().take(3) {
                        let normalized = (val as f32 / 255.0 - MEAN[i]) / STD[i];
                        patch_data.push(normalized);
                    }
                }
            }
            
            patches.extend(patch_data);
        }
    }
    
    // Add CLS token patch (zeros)
    let cls_patch = vec![0.0f32; patch_size * patch_size * 3];
    patches.extend(cls_patch);
    
    let num_patches_total = num_patches * num_patches + 1; // +1 for CLS
    Ok(Tensor::from_vec(
        patches,
        &[1, num_patches_total, patch_size * patch_size * 3],
        device
    )?)
}

/// Unified memory manager for Apple Silicon
pub struct UnifiedMemoryManager {
    device: Device,
    cached_tensors: Vec<(String, Tensor)>,
}

impl UnifiedMemoryManager {
    pub fn new() -> Result<Self> {
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);
        Ok(Self {
            device,
            cached_tensors: Vec::new(),
        })
    }
    
    /// Zero-copy transfer between CPU and Metal on M1/M2/M3
    pub fn transfer(&self, tensor: &Tensor, target_device: &Device) -> Result<Tensor> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            // On Apple Silicon, CPU and GPU share unified memory
            // This is effectively a no-op, just updating metadata
            match (&tensor.device(), target_device) {
                (Device::Cpu, Device::Metal(_)) | 
                (Device::Metal(_), Device::Cpu) => {
                    // Unified memory - no actual copy needed
                    Ok(tensor.to_device(target_device)?)
                }
                _ => Ok(tensor.clone()),
            }
        }
        
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            // Non-Apple Silicon - regular copy
            tensor.to_device(target_device)
        }
    }
    
    /// Cache tensor for reuse
    pub fn cache(&mut self, key: String, tensor: Tensor) {
        // Limit cache size
        if self.cached_tensors.len() > 100 {
            self.cached_tensors.remove(0);
        }
        self.cached_tensors.push((key, tensor));
    }
    
    /// Retrieve cached tensor
    pub fn get_cached(&self, key: &str) -> Option<&Tensor> {
        self.cached_tensors.iter()
            .find(|(k, _)| k == key)
            .map(|(_, t)| t)
    }
}