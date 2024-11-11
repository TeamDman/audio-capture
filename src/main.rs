use anyhow::Result;
use chrono::Utc;
use std::io::Write;
use std::io::{self};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tracing::error;
use tracing::info;
use windows::Win32::Media::Audio::eConsole;
use windows::Win32::Media::Audio::eRender;
use windows::Win32::Media::Audio::IAudioCaptureClient;
use windows::Win32::Media::Audio::IAudioClient;
use windows::Win32::Media::Audio::IMMDeviceEnumerator;
use windows::Win32::Media::Audio::MMDeviceEnumerator;
use windows::Win32::Media::Audio::AUDCLNT_SHAREMODE_SHARED;
use windows::Win32::Media::Audio::AUDCLNT_STREAMFLAGS_LOOPBACK;
use windows::Win32::Media::Audio::WAVEFORMATEX;
use windows::Win32::Media::Audio::WAVE_FORMAT_PCM;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Com::COINIT_MULTITHREADED;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting application");

    // Initialize COM library
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }

    let is_recording = Arc::new(Mutex::new(false));
    let audio_data = Arc::new(Mutex::new(Vec::new()));
    let mix_format = Arc::new(Mutex::new(None));

    let is_recording_clone = Arc::clone(&is_recording);
    let audio_data_clone = Arc::clone(&audio_data);
    let mix_format_clone = Arc::clone(&mix_format);

    let _capture_thread = thread::spawn(move || {
        if let Err(e) = capture_audio(is_recording_clone, audio_data_clone, mix_format_clone) {
            error!("Capture thread error: {:?}", e);
        }
    });

    // Main loop
    let stdin = io::stdin();
    loop {
        println!("Press Enter to start/stop recording...");
        std::io::stdout().flush()?;
        let mut input = String::new();
        stdin.read_line(&mut input)?;

        let mut is_recording_lock = is_recording.lock().unwrap();

        let current_state = if *is_recording_lock {
            // Stop recording
            *is_recording_lock = false;
            info!("State changed to Idle");

            // Save audio data to file
            let mut audio_data_lock = audio_data.lock().unwrap();

            if audio_data_lock.is_empty() {
                info!("No audio data captured");
            } else {
                // Get the mix_format
                let mix_format_value = {
                    let mix_format_lock = mix_format.lock().unwrap();
                    *mix_format_lock
                };

                if let Some(mix_format_value) = mix_format_value {
                    // Save to file
                    let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
                    let filename = format!("captures/{}.wav", timestamp);

                    // Ensure captures directory exists
                    std::fs::create_dir_all("captures")?;

                    // Save as WAV file
                    match save_as_wav(&audio_data_lock, &filename, &mix_format_value) {
                        Ok(_) => info!("Audio saved to {}", filename),
                        Err(e) => error!("Failed to save audio: {:?}", e),
                    }
                } else {
                    error!("Mix format not available");
                }
            }

            // Clear audio data
            audio_data_lock.clear();

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

fn capture_audio(
    is_recording: Arc<Mutex<bool>>,
    audio_data: Arc<Mutex<Vec<u8>>>,
    mix_format: Arc<Mutex<Option<WAVEFORMATEX>>>,
) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

        let audio_client: IAudioClient = device.Activate::<IAudioClient>(CLSCTX_ALL, None)?;

        let mix_format_ptr = audio_client.GetMixFormat()?;
        let mix_format_value = *mix_format_ptr;

        // Store mix_format for use in main thread
        {
            let mut mix_format_lock = mix_format.lock().unwrap();
            *mix_format_lock = Some(mix_format_value);
        }

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

        // Add a variable to track the previous recording state
        let mut was_recording = false;

        loop {
            let recording = { *is_recording.lock().unwrap() };

            if recording && !was_recording {
                // Transition from not recording to recording
                audio_client.Start()?;
                was_recording = true;
                info!("Audio client started");
            } else if !recording && was_recording {
                // Transition from recording to not recording
                audio_client.Stop()?;
                was_recording = false;
                info!("Audio client stopped");
            }

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

                let bytes_per_frame = mix_format_value.nBlockAlign as usize;
                let data_size = (num_frames_available as usize) * bytes_per_frame;

                let data_slice = std::slice::from_raw_parts(data_ptr, data_size);

                audio_data.lock().unwrap().extend_from_slice(data_slice);

                capture_client.ReleaseBuffer(num_frames_available)?;

                packet_length = capture_client.GetNextPacketSize()?;
            }

            thread::sleep(Duration::from_millis(10));
        }

        // Uncomment if you plan to exit the loop and need to clean up
        // audio_client.Stop()?;
        // CoUninitialize();
    }
}

fn save_as_wav(audio_data: &[u8], filename: &str, mix_format: &WAVEFORMATEX) -> Result<()> {
    use hound::WavSpec;

    let spec = WavSpec {
        channels: mix_format.nChannels,
        sample_rate: mix_format.nSamplesPerSec,
        bits_per_sample: mix_format.wBitsPerSample,
        sample_format: if mix_format.wFormatTag == WAVE_FORMAT_PCM as u16 {
            hound::SampleFormat::Int
        } else {
            hound::SampleFormat::Float
        },
    };

    let mut writer = hound::WavWriter::create(filename, spec)?;

    // Handle sample conversion based on bits per sample and format
    // For example, if bits_per_sample == 16 and sample_format == Int
    let bytes_per_sample = (mix_format.wBitsPerSample / 8) as usize;

    let samples =
        audio_data
            .chunks_exact(bytes_per_sample)
            .map(|b| match mix_format.wBitsPerSample {
                16 => i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32,
                24 => {
                    let value = i32::from_le_bytes([b[0], b[1], b[2], 0]);
                    value as f32 / 8388607.0 // 24-bit max value
                }
                32 => {
                    if mix_format.wFormatTag == WAVE_FORMAT_PCM as u16 {
                        let value = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        value as f32 / i32::MAX as f32
                    } else {
                        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
                    }
                }
                _ => 0.0,
            });

    for sample in samples {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;
    Ok(())
}
