# Chonker7 Machine Learning Setup

## Overview

Chonker7 v7.41+ includes optional machine learning capabilities powered by LayoutLMv3 for intelligent document understanding. This uses Apple's Neural Engine (ANE) on M1/M2/M3 Macs for 10-20x faster inference than CPU.

## Features

- **Intelligent Entity Extraction**: Detects dates, values, organizations, and form fields
- **Table Structure Recognition**: Understands table layouts and relationships
- **Document Classification**: Identifies document types (forms, invoices, reports)
- **Spatial Understanding**: Leverages character positions for better accuracy
- **ANE Acceleration**: 10-20ms per page on Apple Silicon

## Prerequisites

### Hardware
- **Recommended**: Apple M1/M2/M3 Mac (for ANE acceleration)
- **Supported**: Any 64-bit macOS or Linux system (CPU inference)

### Software
- Rust 1.75+
- Python 3.9+ (for model conversion)
- 2GB free disk space for models

## Installation

### 1. Download Pre-trained Models

```bash
# Create model directory
mkdir -p ~/.cache/chonker7/models

# Download LayoutLMv3 base model (400MB)
cd ~/.cache/chonker7/models
curl -L https://huggingface.co/microsoft/layoutlmv3-base/resolve/main/model.safetensors \
     -o layoutlmv3/model.safetensors
curl -L https://huggingface.co/microsoft/layoutlmv3-base/resolve/main/tokenizer.json \
     -o layoutlmv3/tokenizer.json
```

### 2. Convert Models for CoreML (Apple Silicon only)

```bash
# Install conversion tools
pip install coremltools transformers torch

# Run conversion script
python scripts/convert_to_coreml.py \
    --model ~/.cache/chonker7/models/layoutlmv3 \
    --output ~/.cache/chonker7/models/layoutlmv3
```

### 3. Build with ML Features

```bash
# Apple Silicon with ANE acceleration
cargo build --release --features "ml,coreml"

# Metal GPU acceleration (fallback)
cargo build --release --features "ml,metal"

# CPU-only (all platforms)
cargo build --release --features "ml"
```

## Usage

### Enable ML Processing

Press `Ctrl+M` in Chonker7 to toggle ML-enhanced extraction. When enabled:

- TEXT tab shows entity markers (`$` for values, `@` for dates, etc.)
- READER tab displays structured document understanding
- Status bar shows inference backend (ANE/Metal/CPU)

### Performance Benchmarks

| Backend | Hardware | Speed (512 tokens) | Memory |
|---------|----------|-------------------|---------|
| ANE | M2 Pro | 12ms | 200MB |
| Metal | M2 Pro | 35ms | 400MB |
| CPU | M2 Pro | 150ms | 600MB |
| CPU | Intel i7 | 400ms | 800MB |

### Chunking for Long Documents

Documents over 512 tokens are automatically chunked with 64-token overlap:

```rust
// Automatic chunking configuration
ChunkConfig {
    max_tokens: 512,      // Optimal for ANE
    overlap_tokens: 64,   // Context preservation
    stride: 448,          // Sliding window
}
```

## Model Architecture

### LayoutLMv3 Components

1. **Text Encoder**: Processes OCR text with positional embeddings
2. **Visual Encoder**: Vision Transformer for page images (optional)
3. **Spatial Encoder**: 2D position embeddings from bounding boxes
4. **Cross-Modal Fusion**: Combines text, visual, and spatial features

### CoreML Optimization

The model is split into three CoreML components for optimal ANE utilization:

- `text_encoder.mlmodelc`: Text + spatial processing
- `visual_encoder.mlmodelc`: Image patch extraction
- `cross_modal.mlmodelc`: Feature fusion

## Troubleshooting

### "CoreML model not found"
- Run the conversion script to generate `.mlmodelc` files
- Ensure models are in `~/.cache/chonker7/models/layoutlmv3/`

### "ANE not available, falling back to CPU"
- ANE requires Apple M1/M2/M3 processor
- Check Activity Monitor > GPU History for ANE usage

### High memory usage
- Reduce chunk size: `export CHONKER_CHUNK_SIZE=256`
- Disable visual encoder: `export CHONKER_TEXT_ONLY=1`

### Slow inference
- Ensure you built with `--release` flag
- Check if using correct backend (ANE > Metal > CPU)
- Consider upgrading to Apple Silicon Mac

## Advanced Configuration

### Environment Variables

```bash
# Model paths
export CHONKER_MODEL_PATH="~/.cache/chonker7/models/layoutlmv3"

# Performance tuning
export CHONKER_CHUNK_SIZE=512
export CHONKER_BATCH_SIZE=1
export CHONKER_USE_VISUAL=1

# Backend selection
export CHONKER_FORCE_CPU=0
export CHONKER_FORCE_METAL=0
```

### Custom Models

To use fine-tuned models:

1. Export from HuggingFace to SafeTensors format
2. Place in model directory with `tokenizer.json`
3. Run CoreML conversion script
4. Update `CHONKER_MODEL_PATH`

## Development

### Running Tests

```bash
# Test ML pipeline
cargo test --features ml

# Benchmark inference
cargo bench --features "ml,coreml"
```

### Debugging

```bash
# Enable debug logging
RUST_LOG=chonker7::ml=debug cargo run --features ml

# Profile ANE usage
xcrun xctrace record --template "Neural Engine" --launch target/release/chonker7
```

## License Note

LayoutLMv3 is licensed under CC-BY-NC-SA-4.0 for non-commercial use. 
For commercial use, please refer to Microsoft's licensing terms.