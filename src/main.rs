use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use tracing::{error, info};
use tracing_subscriber;

use windows::{
    core::*,
    Win32::{
        Media::Audio::{
            eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
            MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    },
};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting application");

    // Initialize COM library
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }

    let is_recording = Arc::new(Mutex::new(false));
    let audio_data = Arc::new(Mutex::new(Vec::new()));

    let is_recording_clone = Arc::clone(&is_recording);
    let audio_data_clone = Arc::clone(&audio_data);

    let _capture_thread = thread::spawn(move || {
        if let Err(e) = capture_audio(is_recording_clone, audio_data_clone) {
            error!("Capture thread error: {:?}", e);
        }
    });

    // Main loop
    let stdin = io::stdin();
    println!("Press Enter to start/stop recording...");

    loop {
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;

        let mut is_recording_lock = is_recording.lock().unwrap();

        let current_state = if *is_recording_lock {
            // Stop recording
            *is_recording_lock = false;
            info!("State changed to Idle");

            // Save audio data to file
            let audio_data_lock = audio_data.lock().unwrap();

            if audio_data_lock.is_empty() {
                info!("No audio data captured");
            } else {
                // Save to file
                let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
                let filename = format!("captures/{}.wav", timestamp);

                // Ensure captures directory exists
                std::fs::create_dir_all("captures")?;

                // Save as WAV file
                match save_as_wav(&audio_data_lock, &filename) {
                    Ok(_) => info!("Audio saved to {}", filename),
                    Err(e) => error!("Failed to save audio: {:?}", e),
                }
            }

            // Clear audio data
            audio_data.lock().unwrap().clear();

            "Idle"
        } else {
            // Start recording
            *is_recording_lock = true;
            info!("State changed to Listening");

            "Listening"
        };

        println!("Current state: {}", current_state);
    }

    // Wait for the capture thread to finish (unreachable in this loop)
    // _capture_thread.join().unwrap();

    // unsafe { CoUninitialize(); }

    // Ok(())
}

fn capture_audio(is_recording: Arc<Mutex<bool>>, audio_data: Arc<Mutex<Vec<u8>>>) -> Result<()> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

        let audio_client: IAudioClient = device.Activate::<IAudioClient>(CLSCTX_ALL, None)?;

        let mix_format_ptr = audio_client.GetMixFormat()?;
        let mix_format = *mix_format_ptr;

        let hns_requested_duration = 10000000; // 1 second in 100-ns units

        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            hns_requested_duration,
            0,
            mix_format_ptr,
            None,
        )?;

        let capture_client: IAudioCaptureClient =
            audio_client.GetService::<IAudioCaptureClient>()?;

        audio_client.Start()?;

        loop {
            let recording = { *is_recording.lock().unwrap() };

            if !recording {
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let mut packet_length = capture_client.GetNextPacketSize()?;

            while packet_length > 0 {
                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames_available = 0;
                let mut flags = 0;

                capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut num_frames_available,
                    &mut flags,
                    None,
                    None,
                )?;

                let bytes_per_frame = mix_format.nBlockAlign as usize;
                let data_size = (num_frames_available as usize) * bytes_per_frame;

                let data_slice = std::slice::from_raw_parts(data_ptr, data_size);

                audio_data.lock().unwrap().extend_from_slice(data_slice);

                capture_client.ReleaseBuffer(num_frames_available)?;

                packet_length = capture_client.GetNextPacketSize()?;
            }

            thread::sleep(Duration::from_millis(10));
        }

        // audio_client.Stop()?;
        // CoUninitialize();
    }
}

fn save_as_wav(audio_data: &[u8], filename: &str) -> Result<()> {
    // Use hound crate to write WAV file
    use hound::WavSpec;

    // Assuming 16-bit stereo PCM
    let spec = WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(filename, spec)?;

    // Write samples
    let samples = audio_data
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]));

    for sample in samples {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;

    Ok(())
}
