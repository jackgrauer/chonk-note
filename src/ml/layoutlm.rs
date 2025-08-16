use anyhow::{Result, anyhow};
use candle_core::{Device, Tensor, DType, Module};
use candle_nn::{VarBuilder, Embedding, Linear};
use std::path::PathBuf;

#[cfg(feature = "coreml")]
use crate::ml::coreml_bridge::{CoreMLModel, CoreMLConfig};

use crate::ml::{DocumentProcessor, DocumentUnderstanding};
use crate::content_extractor::{CharacterData, PageData};

// LayoutLMv3 configuration constants
const MAX_POSITION_EMBEDDINGS: usize = 512;
const MAX_2D_POSITION_EMBEDDINGS: usize = 1024;
const HIDDEN_SIZE: usize = 768;
const NUM_ATTENTION_HEADS: usize = 12;
const NUM_HIDDEN_LAYERS: usize = 12;
const VOCAB_SIZE: usize = 50265;
const TYPE_VOCAB_SIZE: usize = 2;
const PATCH_SIZE: usize = 16;

/// CoreML-accelerated LayoutLMv3 implementation
#[cfg(feature = "coreml")]
pub struct LayoutLMv3CoreML {
    text_encoder: CoreMLModel,
    visual_encoder: CoreMLModel,
    cross_modal_encoder: CoreMLModel,
    tokenizer: tokenizers::Tokenizer,
    device: Device,
}

#[cfg(feature = "coreml")]
impl LayoutLMv3CoreML {
    pub async fn load_default() -> Result<Self> {
        Self::load(&Self::default_model_path()?).await
    }
    
    pub async fn load(model_path: &str) -> Result<Self> {
        // Text encoder configuration
        let text_config = CoreMLConfig {
            input_names: vec!["input_ids".to_string(), "bbox".to_string()],
            output_name: "text_hidden".to_string(),
            input_shapes: vec![(1, 512), (1, 512, 4)], // batch, seq_len, bbox_dims
        };
        
        // Visual encoder for image patches
        let visual_config = CoreMLConfig {
            input_names: vec!["pixel_values".to_string()],
            output_name: "visual_hidden".to_string(),
            input_shapes: vec![(1, 3, 224, 224)], // batch, channels, height, width
        };
        
        // Cross-modal fusion
        let cross_modal_config = CoreMLConfig {
            input_names: vec!["text_hidden".to_string(), "visual_hidden".to_string()],
            output_name: "fused_output".to_string(),
            input_shapes: vec![(1, 512, 768), (1, 197, 768)], // text and visual hidden states
        };
        
        // Load pre-compiled CoreML models
        let text_encoder = CoreMLModel::load(
            &format!("{}/text_encoder.mlmodelc", model_path),
            text_config
        )?;
        
        let visual_encoder = CoreMLModel::load(
            &format!("{}/visual_encoder.mlmodelc", model_path),
            visual_config
        )?;
        
        let cross_modal_encoder = CoreMLModel::load(
            &format!("{}/cross_modal.mlmodelc", model_path),
            cross_modal_config
        )?;
        
        // Load tokenizer
        let tokenizer = tokenizers::Tokenizer::from_file(
            format!("{}/tokenizer.json", model_path)
        ).map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;
        
        Ok(Self {
            text_encoder,
            visual_encoder,
            cross_modal_encoder,
            tokenizer,
            device: Device::new_metal(0).unwrap_or(Device::Cpu),
        })
    }
    
    fn default_model_path() -> Result<String> {
        let home = std::env::var("HOME")?;
        Ok(format!("{}/.cache/chonker7/models/layoutlmv3", home))
    }
    
    fn prepare_inputs(&self, page_data: &PageData) -> Result<(Tensor, Tensor, Option<Tensor>)> {
        // Tokenize text
        let text: String = page_data.characters.iter()
            .map(|c| c.unicode)
            .collect();
        
        let encoding = self.tokenizer.encode(text, false)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;
        
        // Create input_ids tensor
        let input_ids = Tensor::from_vec(
            encoding.get_ids().to_vec(),
            &[1, encoding.get_ids().len()],
            &self.device
        )?;
        
        // Create bbox tensor (normalized coordinates)
        let mut bboxes = Vec::new();
        for char in &page_data.characters {
            bboxes.push(Self::normalize_bbox(
                char.x,
                char.y,
                char.x + char.width,
                char.y + char.height,
                page_data.page_width,
                page_data.page_height,
            ));
        }
        
        let bbox_tensor = Tensor::from_vec(
            bboxes.into_iter().flatten().collect(),
            &[1, page_data.characters.len(), 4],
            &self.device
        )?;
        
        // Visual features (if page image available)
        let visual_tensor = if let Some(image) = &page_data.page_image {
            Some(self.prepare_visual_patches(image)?)
        } else {
            None
        };
        
        Ok((input_ids, bbox_tensor, visual_tensor))
    }
    
    fn normalize_bbox(x1: f32, y1: f32, x2: f32, y2: f32, width: f32, height: f32) -> Vec<f32> {
        vec![
            (x1 * 1000.0 / width).round(),
            (y1 * 1000.0 / height).round(),
            (x2 * 1000.0 / width).round(),
            (y2 * 1000.0 / height).round(),
        ]
    }
    
    fn prepare_visual_patches(&self, image: &image::DynamicImage) -> Result<Tensor> {
        // Resize to 224x224 for ViT
        let resized = image.resize_exact(224, 224, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();
        
        // Normalize with ImageNet stats
        let mut pixels = Vec::new();
        for pixel in rgb.pixels() {
            pixels.push((pixel[0] as f32 / 255.0 - 0.485) / 0.229); // R
            pixels.push((pixel[1] as f32 / 255.0 - 0.456) / 0.224); // G
            pixels.push((pixel[2] as f32 / 255.0 - 0.406) / 0.225); // B
        }
        
        Tensor::from_vec(pixels, &[1, 3, 224, 224], &self.device)
    }
}

#[cfg(feature = "coreml")]
impl DocumentProcessor for LayoutLMv3CoreML {
    fn process_page(&self, page_data: &PageData) -> Result<DocumentUnderstanding> {
        let (input_ids, bbox, visual) = self.prepare_inputs(page_data)?;
        
        // Text + spatial encoding
        let text_hidden = self.text_encoder.forward(&[&input_ids, &bbox])?;
        
        // Visual encoding (if available)
        let outputs = if let Some(visual_tensor) = visual {
            let visual_hidden = self.visual_encoder.forward(&[&visual_tensor])?;
            
            // Cross-modal fusion
            self.cross_modal_encoder.forward(&[&text_hidden, &visual_hidden])?
        } else {
            // Text-only processing
            text_hidden
        };
        
        // Decode to document understanding
        self.decode_outputs(outputs, page_data)
    }
    
    fn backend_name(&self) -> &str {
        "LayoutLMv3-CoreML-ANE"
    }
}

/// Native Candle implementation (CPU/Metal fallback)
pub struct LayoutLMv3Native {
    word_embeddings: Embedding,
    position_embeddings: Embedding,
    x_position_embeddings: Embedding,
    y_position_embeddings: Embedding,
    h_position_embeddings: Embedding,
    w_position_embeddings: Embedding,
    token_type_embeddings: Embedding,
    layer_norm: candle_nn::LayerNorm,
    encoder: TransformerEncoder,
    device: Device,
    tokenizer: tokenizers::Tokenizer,
}

impl LayoutLMv3Native {
    pub async fn load_default() -> Result<Self> {
        let model_path = Self::default_model_path()?;
        Self::load(&model_path).await
    }
    
    pub async fn load(model_path: &str) -> Result<Self> {
        let device = Self::select_device()?;
        
        // Load weights from safetensors
        let weights_path = format!("{}/model.safetensors", model_path);
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[weights_path],
                DType::F32,
                &device
            )?
        };
        
        // Initialize embeddings
        let word_embeddings = Embedding::new(
            vb.get((VOCAB_SIZE, HIDDEN_SIZE), "embeddings.word_embeddings.weight")?,
            HIDDEN_SIZE
        );
        
        let position_embeddings = Embedding::new(
            vb.get((MAX_POSITION_EMBEDDINGS, HIDDEN_SIZE), "embeddings.position_embeddings.weight")?,
            HIDDEN_SIZE
        );
        
        // 2D position embeddings for bbox
        let x_position_embeddings = Embedding::new(
            vb.get((MAX_2D_POSITION_EMBEDDINGS, HIDDEN_SIZE/4), "embeddings.x_position_embeddings.weight")?,
            HIDDEN_SIZE/4
        );
        
        let y_position_embeddings = Embedding::new(
            vb.get((MAX_2D_POSITION_EMBEDDINGS, HIDDEN_SIZE/4), "embeddings.y_position_embeddings.weight")?,
            HIDDEN_SIZE/4
        );
        
        let h_position_embeddings = Embedding::new(
            vb.get((MAX_2D_POSITION_EMBEDDINGS, HIDDEN_SIZE/4), "embeddings.h_position_embeddings.weight")?,
            HIDDEN_SIZE/4
        );
        
        let w_position_embeddings = Embedding::new(
            vb.get((MAX_2D_POSITION_EMBEDDINGS, HIDDEN_SIZE/4), "embeddings.w_position_embeddings.weight")?,
            HIDDEN_SIZE/4
        );
        
        let token_type_embeddings = Embedding::new(
            vb.get((TYPE_VOCAB_SIZE, HIDDEN_SIZE), "embeddings.token_type_embeddings.weight")?,
            HIDDEN_SIZE
        );
        
        let layer_norm = candle_nn::layer_norm(
            HIDDEN_SIZE,
            candle_nn::LayerNormConfig::default(),
            vb.pp("embeddings.LayerNorm")
        )?;
        
        // Load transformer encoder
        let encoder = TransformerEncoder::load(vb.pp("encoder"), NUM_HIDDEN_LAYERS)?;
        
        // Load tokenizer
        let tokenizer = tokenizers::Tokenizer::from_file(
            format!("{}/tokenizer.json", model_path)
        ).map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;
        
        Ok(Self {
            word_embeddings,
            position_embeddings,
            x_position_embeddings,
            y_position_embeddings,
            h_position_embeddings,
            w_position_embeddings,
            token_type_embeddings,
            layer_norm,
            encoder,
            device,
            tokenizer,
        })
    }
    
    fn default_model_path() -> Result<String> {
        let home = std::env::var("HOME")?;
        Ok(format!("{}/.cache/chonker7/models/layoutlmv3", home))
    }
    
    fn select_device() -> Result<Device> {
        #[cfg(feature = "metal")]
        {
            if let Ok(device) = Device::new_metal(0) {
                eprintln!("Using Metal backend");
                return Ok(device);
            }
        }
        eprintln!("Using CPU backend");
        Ok(Device::Cpu)
    }
    
    fn forward(&self, input_ids: &Tensor, bbox: &Tensor) -> Result<Tensor> {
        let seq_len = input_ids.dim(1)?;
        
        // Word embeddings
        let word_embeds = self.word_embeddings.forward(input_ids)?;
        
        // Position embeddings
        let position_ids = Tensor::arange(0u32, seq_len as u32, &self.device)?
            .unsqueeze(0)?;
        let position_embeds = self.position_embeddings.forward(&position_ids)?;
        
        // Spatial embeddings from bbox
        // Extract each coordinate from bbox tensor [batch, seq_len, 4]
        let x_coords = bbox.narrow(2, 0, 1)?.squeeze(2)?;
        let y_coords = bbox.narrow(2, 1, 1)?.squeeze(2)?;
        let x2_coords = bbox.narrow(2, 2, 1)?.squeeze(2)?;
        let y2_coords = bbox.narrow(2, 3, 1)?.squeeze(2)?;
        
        // Convert to embeddings
        let x_embeds = self.x_position_embeddings.forward(&x_coords)?;
        let y_embeds = self.y_position_embeddings.forward(&y_coords)?;
        let h_embeds = self.h_position_embeddings.forward(&x2_coords)?;
        let w_embeds = self.w_position_embeddings.forward(&y2_coords)?;
        
        // Concatenate spatial embeddings
        let spatial_embeds = Tensor::cat(&[x_embeds, y_embeds, h_embeds, w_embeds], 2)?;
        
        // Combine all embeddings
        let embeddings = (word_embeds + position_embeds + spatial_embeds)?;
        let embeddings = self.layer_norm.forward(&embeddings)?;
        
        // Run through transformer
        self.encoder.forward(&embeddings)
    }
}

impl DocumentProcessor for LayoutLMv3Native {
    fn process_page(&self, page_data: &PageData) -> Result<DocumentUnderstanding> {
        // Prepare inputs
        let text: String = page_data.characters.iter()
            .map(|c| c.unicode)
            .collect();
        
        let encoding = self.tokenizer.encode(text, false)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;
        
        let input_ids = Tensor::from_vec(
            encoding.get_ids().to_vec(),
            &[1, encoding.get_ids().len()],
            &self.device
        )?;
        
        // Prepare bbox tensor
        let mut bboxes = Vec::new();
        for char in &page_data.characters {
            let normalized = Self::normalize_bbox(
                char.x,
                char.y, 
                char.x + char.width,
                char.y + char.height,
                page_data.page_width,
                page_data.page_height,
            );
            bboxes.extend(normalized);
        }
        
        let bbox = Tensor::from_vec(
            bboxes,
            &[1, page_data.characters.len(), 4],
            &self.device
        )?;
        
        // Forward pass
        let outputs = self.forward(&input_ids, &bbox)?;
        
        // Decode to document understanding
        self.decode_outputs(outputs, page_data)
    }
    
    fn backend_name(&self) -> &str {
        match self.device {
            Device::Metal(_) => "LayoutLMv3-Native-Metal",
            Device::Cpu => "LayoutLMv3-Native-CPU",
            _ => "LayoutLMv3-Native",
        }
    }
}

impl LayoutLMv3Native {
    fn normalize_bbox(x1: f32, y1: f32, x2: f32, y2: f32, width: f32, height: f32) -> Vec<f32> {
        vec![
            ((x1 * 1000.0 / width).round() as i32).clamp(0, 1000) as f32,
            ((y1 * 1000.0 / height).round() as i32).clamp(0, 1000) as f32,
            ((x2 * 1000.0 / width).round() as i32).clamp(0, 1000) as f32,
            ((y2 * 1000.0 / height).round() as i32).clamp(0, 1000) as f32,
        ]
    }
    
    fn decode_outputs(&self, outputs: Tensor, page_data: &PageData) -> Result<DocumentUnderstanding> {
        // Extract hidden states
        let hidden_states = outputs.to_vec2::<f32>()?;
        
        // Simple heuristic-based decoding for now
        // In production, you'd have task-specific heads (NER, relation extraction, etc.)
        let mut doc = DocumentUnderstanding::new();
        
        // Detect entities based on attention patterns
        // This is simplified - real implementation would use trained classification heads
        for (i, char_data) in page_data.characters.iter().enumerate() {
            if i < hidden_states[0].len() {
                let activation = hidden_states[0][i];
                
                // Threshold-based entity detection (placeholder)
                if activation > 0.8 {
                    doc.add_entity(char_data.clone(), crate::ml::EntityType::Organization);
                } else if activation > 0.6 {
                    doc.add_entity(char_data.clone(), crate::ml::EntityType::Value);
                }
            }
        }
        
        Ok(doc)
    }
}

/// Transformer encoder block
struct TransformerEncoder {
    layers: Vec<TransformerLayer>,
}

impl TransformerEncoder {
    fn load(vb: VarBuilder, num_layers: usize) -> Result<Self> {
        let mut layers = Vec::new();
        for i in 0..num_layers {
            layers.push(TransformerLayer::load(vb.pp(&format!("layer.{}", i)))?);
        }
        Ok(Self { layers })
    }
    
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let mut hidden = x.clone();
        for layer in &self.layers {
            hidden = layer.forward(&hidden)?;
        }
        Ok(hidden)
    }
}

/// Single transformer layer
struct TransformerLayer {
    attention: MultiHeadAttention,
    intermediate: Linear,
    output: Linear,
    ln1: candle_nn::LayerNorm,
    ln2: candle_nn::LayerNorm,
}

impl TransformerLayer {
    fn load(vb: VarBuilder) -> Result<Self> {
        let attention = MultiHeadAttention::load(vb.pp("attention"))?;
        let intermediate = candle_nn::linear(
            HIDDEN_SIZE,
            HIDDEN_SIZE * 4,
            vb.pp("intermediate.dense")
        )?;
        let output = candle_nn::linear(
            HIDDEN_SIZE * 4,
            HIDDEN_SIZE,
            vb.pp("output.dense")
        )?;
        let ln1 = candle_nn::layer_norm(
            HIDDEN_SIZE,
            candle_nn::LayerNormConfig::default(),
            vb.pp("attention.output.LayerNorm")
        )?;
        let ln2 = candle_nn::layer_norm(
            HIDDEN_SIZE,
            candle_nn::LayerNormConfig::default(),
            vb.pp("output.LayerNorm")
        )?;
        
        Ok(Self {
            attention,
            intermediate,
            output,
            ln1,
            ln2,
        })
    }
    
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Self-attention with residual
        let attn_out = self.attention.forward(x)?;
        let x = self.ln1.forward(&(x + attn_out)?)?;
        
        // FFN with residual
        let ffn_out = self.intermediate.forward(&x)?;
        let ffn_out = ffn_out.gelu()?;
        let ffn_out = self.output.forward(&ffn_out)?;
        
        Ok(self.ln2.forward(&(x + ffn_out)?)?)
    }
}

/// Multi-head attention
struct MultiHeadAttention {
    query: Linear,
    key: Linear,
    value: Linear,
    output: Linear,
    num_heads: usize,
    head_dim: usize,
}

impl MultiHeadAttention {
    fn load(vb: VarBuilder) -> Result<Self> {
        let query = candle_nn::linear(HIDDEN_SIZE, HIDDEN_SIZE, vb.pp("self.query"))?;
        let key = candle_nn::linear(HIDDEN_SIZE, HIDDEN_SIZE, vb.pp("self.key"))?;
        let value = candle_nn::linear(HIDDEN_SIZE, HIDDEN_SIZE, vb.pp("self.value"))?;
        let output = candle_nn::linear(HIDDEN_SIZE, HIDDEN_SIZE, vb.pp("output.dense"))?;
        
        Ok(Self {
            query,
            key,
            value,
            output,
            num_heads: NUM_ATTENTION_HEADS,
            head_dim: HIDDEN_SIZE / NUM_ATTENTION_HEADS,
        })
    }
    
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let (batch_size, seq_len, _) = x.dims3()?;
        
        // Linear projections
        let q = self.query.forward(x)?;
        let k = self.key.forward(x)?;
        let v = self.value.forward(x)?;
        
        // Reshape for multi-head attention
        let q = q.reshape(&[batch_size, seq_len, self.num_heads, self.head_dim])?
            .transpose(1, 2)?;
        let k = k.reshape(&[batch_size, seq_len, self.num_heads, self.head_dim])?
            .transpose(1, 2)?;
        let v = v.reshape(&[batch_size, seq_len, self.num_heads, self.head_dim])?
            .transpose(1, 2)?;
        
        // Scaled dot-product attention
        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let scores = q.matmul(&k.transpose(2, 3)?)?.affine(scale, 0.0)?;
        let attn = candle_nn::ops::softmax(&scores, 3)?;
        let context = attn.matmul(&v)?;
        
        // Reshape back
        let context = context.transpose(1, 2)?
            .reshape(&[batch_size, seq_len, HIDDEN_SIZE])?;
        
        Ok(self.output.forward(&context)?)
    }
}