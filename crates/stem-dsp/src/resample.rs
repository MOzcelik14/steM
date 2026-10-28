//! Audio resampling wrapper using rubato

use rubato::{FastFixedIn, Resampler};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResampleError {
    #[error("Resampler construction failed: {0}")]
    Init(#[from] rubato::ResamplerConstructionError),

    #[error("Resampling processing failed: {0}")]
    Process(#[from] rubato::ResampleError),
}

/// Helper to resample multi-channel audio from an input sample rate to a target sample rate.
pub fn resample_audio(
    input_channels: &[Vec<f32>],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<Vec<f32>>, ResampleError> {
    if from_rate == to_rate || input_channels.is_empty() {
        return Ok(input_channels.to_vec());
    }

    let num_channels = input_channels.len();
    let chunk_size = 1024;
    let mut resampler = FastFixedIn::<f32>::new(
        to_rate as f64 / from_rate as f64,
        2.0,
        rubato::PolynomialDegree::Cubic,
        chunk_size,
        num_channels,
    )?;

    let input_len = input_channels[0].len();
    let mut output_channels: Vec<Vec<f32>> = vec![Vec::new(); num_channels];

    let mut pos = 0;
    while pos < input_len {
        let end = (pos + chunk_size).min(input_len);
        let current_chunk_len = end - pos;

        // Prepare chunk (padded with zeroes if at the end)
        let mut chunk_in: Vec<Vec<f32>> = Vec::with_capacity(num_channels);
        for ch in input_channels {
            let mut slice = ch[pos..end].to_vec();
            if slice.len() < chunk_size {
                slice.resize(chunk_size, 0.0);
            }
            chunk_in.push(slice);
        }

        let chunk_out = resampler.process(&chunk_in, None)?;
        for (out_ch, chunk_data) in output_channels.iter_mut().zip(chunk_out) {
            let valid_out_len =
                ((current_chunk_len as f64) * (to_rate as f64 / from_rate as f64)).round() as usize;
            out_ch.extend_from_slice(&chunk_data[..valid_out_len.min(chunk_data.len())]);
        }

        pos += chunk_size;
    }

    Ok(output_channels)
}
