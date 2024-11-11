# Audio Capture in Rust

This project is a Rust application that captures system audio on Windows using the Windows Core Audio APIs. It allows you to start and stop recording by pressing Enter in the console, and saves the captured audio as WAV files with timestamps.

## Features

- **Start/Stop Recording**: Toggle recording state by pressing Enter.
- **System Audio Capture**: Records all system audio output (loopback recording).
- **WAV File Output**: Saves captured audio in high-quality WAV format.
- **Timestamped Filenames**: Automatically names files using the current date and time.
- **Threaded Audio Capture**: Uses multithreading to capture audio without blocking the main thread.
- **Error Handling and Logging**: Provides informative logs and handles errors gracefully.

## Prerequisites

- **Operating System**: Windows 10 or later.
- **Rust Toolchain**: Install Rust from [rust-lang.org](https://www.rust-lang.org/tools/install).

## Dependencies

The project relies on the following Rust crates:

- [`windows`](https://crates.io/crates/windows): Windows API bindings for Rust.
- [`tracing`](https://crates.io/crates/tracing) and [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber): For logging.
- [`chrono`](https://crates.io/crates/chrono): For timestamp formatting.
- [`hound`](https://crates.io/crates/hound): For writing WAV files.
- [`anyhow`](https://crates.io/crates/anyhow): Simplified error handling.

## Installation

1. **Clone the Repository**

   ```bash
   git clone https://github.com/TeamDman/audio-capture.git
   cd audio-capture
   ```

2. **Install Rust**

   If you haven't installed Rust yet, download and install it via [rustup.rs](https://rustup.rs/).

4. **Build the Project**

   ```bash
   cargo build --release
   ```

   This will compile the application in release mode.

## Usage

1. **Run the Application**

   ```bash
   cargo run --release
   ```

2. **Control Recording**

   - **Start Recording**: Press **Enter** when prompted to start recording system audio.
   - **Stop Recording**: Press **Enter** again to stop recording.

3. **Output Files**

   - Recorded audio files are saved in the `captures` directory.
   - Filenames are automatically generated using the current timestamp, e.g., `captures/20241111T181055.wav`.

4. **Example Session**

   ```plaintext
   Press Enter to start/stop recording...

   2024-11-11T18:10:52.737626Z  INFO audio_capture: State changed to Listening
   Current state: Listening
   Press Enter to start/stop recording...
   2024-11-11T18:10:52.787241Z  INFO audio_capture: Audio client started

   2024-11-11T18:10:55.786648Z  INFO audio_capture: State changed to Idle
   2024-11-11T18:10:55.815265Z  INFO audio_capture: Audio saved to captures/20241111T181055.wav
   ```

## Project Structure

- **`src/main.rs`**: The main application source code.
- **`captures/`**: Directory where recorded audio files are saved.
- **`Cargo.toml`**: Project configuration file containing dependencies.

## How It Works

- **COM Initialization**: The application initializes the COM library required for Windows audio APIs.
- **Audio Capture Thread**: Starts a separate thread that handles audio capturing without blocking the main thread.
- **User Interaction**: The main thread waits for user input to start or stop recording.
- **Mutexes and Shared State**: Uses `Arc<Mutex<T>>` to share state between threads safely.
- **WAV File Saving**: Captured audio data is saved using the `hound` crate, which handles WAV file formatting.
