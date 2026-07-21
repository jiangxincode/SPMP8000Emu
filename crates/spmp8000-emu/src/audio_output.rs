use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, OutputCallbackInfo, Sample, SampleFormat, SizedSample, Stream};

pub(crate) struct AudioOutput {
    _stream: Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
    input_rate: u32,
    output_rate: u32,
    output_channels: usize,
    max_queued_samples: usize,
}

impl AudioOutput {
    pub(crate) fn new(input_rate: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default audio output device is available"))?;
        let supported_config = device
            .default_output_config()
            .context("failed to query the default audio output configuration")?;
        let sample_format = supported_config.sample_format();
        let config = supported_config.config();
        let output_rate = config.sample_rate;
        let output_channels = usize::from(config.channels);
        let queue = Arc::new(Mutex::new(VecDeque::new()));

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, Arc::clone(&queue)),
            SampleFormat::I16 => build_stream::<i16>(&device, &config, Arc::clone(&queue)),
            SampleFormat::U16 => build_stream::<u16>(&device, &config, Arc::clone(&queue)),
            format => Err(anyhow!("unsupported audio device sample format {format}")),
        }?;
        stream.play().context("failed to start audio output")?;

        log::info!(
            "Audio output: {} Hz, {} channels, {}",
            output_rate,
            output_channels,
            sample_format
        );

        Ok(Self {
            _stream: stream,
            queue,
            input_rate,
            output_rate,
            output_channels,
            max_queued_samples: output_rate as usize * output_channels / 4,
        })
    }

    pub(crate) fn submit(&self, samples: &[i16]) {
        if samples.len() < 2 || self.input_rate == 0 || self.output_channels == 0 {
            return;
        }

        let input_frames = samples.len() / 2;
        let output_frames = (input_frames as u64 * u64::from(self.output_rate)
            / u64::from(self.input_rate)) as usize;
        let mut converted = Vec::with_capacity(output_frames * self.output_channels);

        for output_frame in 0..output_frames {
            let position = output_frame as f64 * self.input_rate as f64 / self.output_rate as f64;
            let first_frame = (position.floor() as usize).min(input_frames - 1);
            let second_frame = (first_frame + 1).min(input_frames - 1);
            let fraction = (position - first_frame as f64) as f32;
            let left = interpolate(
                samples[first_frame * 2],
                samples[second_frame * 2],
                fraction,
            );
            let right = interpolate(
                samples[first_frame * 2 + 1],
                samples[second_frame * 2 + 1],
                fraction,
            );

            if self.output_channels == 1 {
                converted.push((left + right) * 0.5);
            } else {
                converted.push(left);
                converted.push(right);
                converted.extend(std::iter::repeat_n(
                    (left + right) * 0.5,
                    self.output_channels - 2,
                ));
            }
        }

        let Ok(mut queue) = self.queue.lock() else {
            return;
        };
        let excess = queue
            .len()
            .saturating_add(converted.len())
            .saturating_sub(self.max_queued_samples);
        let drain_count = excess.min(queue.len());
        queue.drain(..drain_count);
        queue.extend(converted);
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Arc<Mutex<VecDeque<f32>>>,
) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let error_callback = |error| log::error!("Audio output error: {}", error);
    device
        .build_output_stream(
            *config,
            move |output: &mut [T], _: &OutputCallbackInfo| write_output(output, &queue),
            error_callback,
            None,
        )
        .context("failed to create audio output stream")
}

fn write_output<T>(output: &mut [T], queue: &Arc<Mutex<VecDeque<f32>>>)
where
    T: Sample + FromSample<f32>,
{
    let Ok(mut queue) = queue.lock() else {
        output.fill(T::from_sample(0.0));
        return;
    };
    for sample in output {
        *sample = T::from_sample(queue.pop_front().unwrap_or(0.0));
    }
}

fn interpolate(first: i16, second: i16, fraction: f32) -> f32 {
    let first = first as f32 / i16::MAX as f32;
    let second = second as f32 / i16::MAX as f32;
    first + (second - first) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_converts_i16_to_normalized_float() {
        assert!((interpolate(0, i16::MAX, 0.5) - 0.5).abs() < f32::EPSILON);
        assert!((interpolate(i16::MIN, i16::MIN, 0.5) + 1.0000305).abs() < f32::EPSILON);
    }

    #[test]
    #[ignore = "requires a host audio output device"]
    fn opens_default_audio_output_device() {
        let output = AudioOutput::new(22_050).unwrap();
        output.submit(&vec![0; 1_470]);
    }
}
