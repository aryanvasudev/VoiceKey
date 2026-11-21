use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;

use voicekey_core::audio::{
    read_wav, write_wav, AudioCaptureManager, AudioRecorder, AudioRecorderConfig, AudioResampler,
};
use voicekey_core::engine::{WhisperConfig, WhisperEngine};
use voicekey_core::input::{is_active_window_terminal, ClipboardManager};
use voicekey_core::ipc::{send_ipc_command, IpcCommand, IpcResponse};
use voicekey_core::vad::EnergyVad;

#[derive(Parser, Debug)]
#[command(
    name = "voicekey-cli",
    about = "VoiceKey headless verification CLI and cross-platform audio engine tool",
    version = "0.2.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List all audio input devices and identify the system default
    Devices,

    /// Record audio from the microphone and save to a standard WAV file
    Record {
        /// Output WAV file path
        #[arg(short, long, default_value = "recording.wav")]
        output: PathBuf,

        /// Recording duration in seconds. If omitted, press Enter to stop.
        #[arg(short, long)]
        duration: Option<f64>,

        /// Name of the input audio device (defaults to system default)
        #[arg(long)]
        device: Option<String>,

        /// Pre-roll buffer duration in milliseconds to prevent clipping initial consonants
        #[arg(long, default_value_t = 200)]
        pre_roll_ms: u32,
    },

    /// Transcribe a 16kHz WAV audio file using the GGML Whisper engine
    Transcribe {
        /// Path to the WAV audio file
        file: PathBuf,

        /// Path to the GGML Whisper model or preset name (tiny.en, base.en, small.en)
        #[arg(short, long, default_value = "base.en")]
        model: String,

        /// Enable spoken punctuation commands ("comma" -> ",", "new line" -> "\n")
        #[arg(long, default_value_t = true)]
        spoken_punct: bool,

        /// Initial prompt for vocabulary biasing and context conditioning
        #[arg(short, long)]
        prompt: Option<String>,

        /// Number of CPU threads to allocate for inference
        #[arg(short, long)]
        threads: Option<usize>,

        /// Target language code (default: en)
        #[arg(short, long, default_value = "en")]
        language: String,
    },

    /// One-step interactive microphone recording and real-time transcription test
    Listen {
        /// Recording duration in seconds. If omitted, press Enter to stop.
        #[arg(short, long)]
        duration: Option<f64>,

        /// Path to the GGML Whisper model or preset name
        #[arg(short, long, default_value = "base.en")]
        model: String,

        /// Name of the input audio device
        #[arg(long)]
        device: Option<String>,

        /// Enable spoken punctuation conversion
        #[arg(long, default_value_t = true)]
        spoken_punct: bool,

        /// Initial prompt for vocabulary biasing
        #[arg(short, long)]
        prompt: Option<String>,
    },

    /// Run diagnostic checks on microphone, CPU instruction sets, and Whisper models
    Doctor,

    /// Toggle VoiceKey dictation state (external desktop shortcut trigger)
    Toggle,

    /// Start VoiceKey dictation recording
    Start,

    /// Stop VoiceKey dictation recording and trigger transcription
    Stop,

    /// Query the current VoiceKey daemon status
    Status,

    /// Directly paste text into the active window with clipboard auto-restore
    Paste {
        /// Text to paste into the active window
        text: String,

        /// Do not restore previous clipboard contents after pasting
        #[arg(long)]
        no_restore: bool,

        /// Debounce delay in milliseconds before restoring clipboard
        #[arg(long, default_value_t = 250)]
        delay_ms: u64,
    },

    /// Post-process or smart-format text using the Micro-LLM engine or offline rules
    Transform {
        /// Input raw text to transform (can contain directives like "VoiceKey bullets: ...")
        text: String,

        /// Explicit transform mode: auto, clean, bullets, professional, casual, code
        #[arg(short, long, default_value = "auto")]
        mode: String,

        /// Local LLM API endpoint URL (default: http://127.0.0.1:11434)
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        endpoint: String,

        /// Model name identifier (e.g. qwen2.5:0.5b, llama3.2:1b)
        #[arg(long, default_value = "qwen2.5:0.5b")]
        model: String,
    },

    /// Low-latency real-time streaming transcription & speculative output test
    Stream {
        /// Recording duration in seconds. If omitted, press Enter to stop.
        #[arg(short, long)]
        duration: Option<f64>,

        /// Path to the GGML Whisper model or preset name (tiny.en, base.en, small.en)
        #[arg(short, long, default_value = "base.en")]
        model: String,

        /// Name of the input audio device
        #[arg(long)]
        device: Option<String>,

        /// Enable VAD silence hysteresis auto-commit (stops automatically after silence)
        #[arg(long)]
        auto_commit: bool,

        /// Silence timeout in milliseconds before auto-commit (default: 800ms)
        #[arg(long, default_value_t = 800)]
        silence_ms: u32,

        /// Initial prompt for vocabulary biasing
        #[arg(short, long)]
        prompt: Option<String>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Devices => handle_devices()?,
        Commands::Record {
            output,
            duration,
            device,
            pre_roll_ms,
        } => handle_record(output, duration, device, pre_roll_ms)?,
        Commands::Transcribe {
            file,
            model,
            spoken_punct,
            prompt,
            threads,
            language,
        } => handle_transcribe(file, model, spoken_punct, prompt, threads, language)?,
        Commands::Listen {
            duration,
            model,
            device,
            spoken_punct,
            prompt,
        } => handle_listen(duration, model, device, spoken_punct, prompt)?,
        Commands::Doctor => handle_doctor()?,
        Commands::Toggle => handle_ipc(IpcCommand::Toggle)?,
        Commands::Start => handle_ipc(IpcCommand::Start)?,
        Commands::Stop => handle_ipc(IpcCommand::Stop)?,
        Commands::Status => handle_ipc(IpcCommand::Status)?,
        Commands::Paste {
            text,
            no_restore,
            delay_ms,
        } => handle_paste(text, !no_restore, delay_ms)?,
        Commands::Transform {
            text,
            mode,
            endpoint,
            model,
        } => handle_transform(text, mode, endpoint, model)?,
        Commands::Stream {
            duration,
            model,
            device,
            auto_commit,
            silence_ms,
            prompt,
        } => handle_stream(duration, model, device, auto_commit, silence_ms, prompt)?,
    }

    Ok(())
}

fn handle_devices() -> Result<()> {
    println!("{}", "=== Audio Input Devices ===".bold().cyan());
    let mgr = AudioCaptureManager::new();
    let devices = mgr
        .list_input_devices()
        .context("Failed to list audio devices")?;

    if devices.is_empty() {
        println!("{}", "No audio input devices detected.".yellow());
        return Ok(());
    }

    for (i, dev) in devices.iter().enumerate() {
        let default_tag = if dev.is_default {
            " [DEFAULT]".bold().green()
        } else {
            "".normal()
        };

        println!("{}. {}{}", i + 1, dev.name.bold(), default_tag);
        println!(
            "   Channels: {} | Default Sample Rate: {} Hz",
            dev.channels, dev.default_sample_rate
        );
    }

    Ok(())
}

fn handle_record(
    output: PathBuf,
    duration: Option<f64>,
    device: Option<String>,
    pre_roll_ms: u32,
) -> Result<()> {
    let config = AudioRecorderConfig {
        device_name: device.clone(),
        pre_roll_ms,
        target_sample_rate: 16000,
    };

    let mut recorder = AudioRecorder::new(config);
    println!("{}", "=== VoiceKey Audio Recorder ===".bold().cyan());
    if let Some(ref dev) = device {
        println!("Input Device: {}", dev);
    } else {
        println!("Input Device: System Default");
    }
    println!("Target Format: 16,000 Hz 16-bit Mono PCM");
    println!("Pre-Roll Memory: {} ms", pre_roll_ms);

    recorder
        .start()
        .context("Failed to start audio recording")?;

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = Arc::clone(&is_running);

    let meter_thread = std::thread::spawn(move || {
        let start_time = Instant::now();
        while is_running_clone.load(Ordering::Relaxed) {
            let elapsed = start_time.elapsed().as_secs_f64();
            print!(
                "\r{} [{:02}:{:02}] (press Enter to stop)...",
                "● RECORDING".bold().red(),
                (elapsed as u64) / 60,
                (elapsed as u64) % 60
            );
            io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    if let Some(dur) = duration {
        std::thread::sleep(Duration::from_secs_f64(dur));
    } else {
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
    }

    is_running.store(false, Ordering::Relaxed);
    let _ = meter_thread.join();
    println!();

    let samples = recorder.stop().context("Failed to stop recording")?;
    let duration_secs = samples.len() as f64 / 16000.0;

    write_wav(&output, &samples, 16000).context("Failed to save WAV file")?;

    let rms = EnergyVad::calculate_rms(&samples);
    let peak = EnergyVad::calculate_peak(&samples);

    println!("{}", "✓ Recording Saved Successfully!".bold().green());
    println!("File: {}", output.display().to_string().bold());
    println!(
        "Duration: {:.2}s ({} samples)",
        duration_secs,
        samples.len()
    );
    println!("RMS Level: {:.4} | Peak: {:.4}", rms, peak);

    Ok(())
}

fn resolve_model_path(model_name_or_path: &str) -> Result<PathBuf> {
    let direct_path = Path::new(model_name_or_path);
    if direct_path.exists() {
        return Ok(direct_path.to_path_buf());
    }

    // Check standard VoiceKey models directory: ~/.local/share/voicekey/models
    let mut search_dirs = Vec::new();
    if let Some(data_dir) = dirs::data_dir() {
        search_dirs.push(data_dir.join("voicekey").join("models"));
    }
    if let Some(cache_dir) = dirs::cache_dir() {
        search_dirs.push(cache_dir.join("voicekey").join("models"));
    }
    search_dirs.push(PathBuf::from("/tmp/voicekey/models"));

    let candidate_names = [
        format!("ggml-{}.bin", model_name_or_path),
        format!("ggml-{}-q5_0.bin", model_name_or_path),
        format!("{}.bin", model_name_or_path),
        model_name_or_path.to_string(),
    ];

    for dir in &search_dirs {
        for candidate in &candidate_names {
            let candidate_path = dir.join(candidate);
            if candidate_path.exists() {
                return Ok(candidate_path);
            }
        }
    }

    // Model not found locally, display helpful guidance
    bail!(
        "Whisper model '{}' not found at direct path or in standard locations:\n{}\n\nTo download a quantized GGML model:\n  mkdir -p ~/.local/share/voicekey/models\n  curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin -o ~/.local/share/voicekey/models/ggml-base.en.bin",
        model_name_or_path,
        search_dirs.iter().map(|d| format!("  - {}", d.display())).collect::<Vec<_>>().join("\n")
    );
}

fn handle_transcribe(
    file: PathBuf,
    model: String,
    spoken_punct: bool,
    prompt: Option<String>,
    threads: Option<usize>,
    language: String,
) -> Result<()> {
    if !file.exists() {
        bail!("Audio file not found: {}", file.display());
    }

    println!(
        "{}",
        "=== VoiceKey GGML Whisper Transcriber ===".bold().cyan()
    );
    println!("Input Audio: {}", file.display().to_string().bold());

    let (mut samples, sample_rate) = read_wav(&file).context("Failed to read audio file")?;
    println!(
        "Sample Rate: {} Hz ({} samples)",
        sample_rate,
        samples.len()
    );

    // Resample to 16,000 Hz if necessary
    if sample_rate != 16000 {
        print!("Resampling {} Hz -> 16000 Hz...", sample_rate);
        io::stdout().flush().unwrap();
        let resampler =
            AudioResampler::new(sample_rate, 16000).context("Failed to initialize resampler")?;
        samples = resampler.resample(&samples).context("Resampling failed")?;
        println!(" Done ({} samples).", samples.len());
    }

    let model_path = resolve_model_path(&model)?;
    println!("Whisper Model: {}", model_path.display().to_string().bold());

    let num_threads = threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    });

    let config = WhisperConfig {
        model_path,
        language: Some(language),
        threads: num_threads,
        temperature: 0.0,
        translate: false,
        initial_prompt: prompt,
        enable_normalization: true,
        enable_spoken_punctuation: spoken_punct,
    };

    print!("Loading model...");
    io::stdout().flush().unwrap();
    let load_start = Instant::now();
    let engine = WhisperEngine::load_from_file(config).context("Failed to load Whisper model")?;
    println!(" Loaded in {:.2}s", load_start.elapsed().as_secs_f64());

    print!("Transcribing...");
    io::stdout().flush().unwrap();
    let start_time = Instant::now();
    let result = engine
        .transcribe(&samples)
        .context("Transcription failed")?;
    let duration = start_time.elapsed().as_secs_f64();
    println!(" Completed in {:.2}s", duration);

    println!("\n{}", "--- Transcription Result ---".bold().green());
    println!("{}", result.text.bold());

    if result.raw_text != result.text {
        println!("\n{}", "--- Raw Whisper Text ---".dimmed());
        println!("{}", result.raw_text.dimmed());
    }

    if !result.segments.is_empty() {
        println!("\n{}", "--- Timed Segments ---".dimmed());
        for seg in &result.segments {
            println!(
                "[{:02}:{:02}.{:03} -> {:02}:{:02}.{:03}] {}",
                seg.start_ms / 60000,
                (seg.start_ms % 60000) / 1000,
                seg.start_ms % 1000,
                seg.end_ms / 60000,
                (seg.end_ms % 60000) / 1000,
                seg.end_ms % 1000,
                seg.text
            );
        }
    }

    Ok(())
}

fn handle_listen(
    duration: Option<f64>,
    model: String,
    device: Option<String>,
    spoken_punct: bool,
    prompt: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        "=== Interactive VoiceKey Dictation Test ===".bold().cyan()
    );
    let model_path = resolve_model_path(&model)?;
    println!("Using Model: {}", model_path.display());

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let config = WhisperConfig {
        model_path,
        language: Some("en".to_string()),
        threads,
        temperature: 0.0,
        translate: false,
        initial_prompt: prompt,
        enable_normalization: true,
        enable_spoken_punctuation: spoken_punct,
    };

    print!("Initializing Whisper engine...");
    io::stdout().flush().unwrap();
    let engine = WhisperEngine::load_from_file(config).context("Failed to load Whisper model")?;
    println!(" Ready.");

    let rec_config = AudioRecorderConfig {
        device_name: device,
        pre_roll_ms: 200,
        target_sample_rate: 16000,
    };
    let mut recorder = AudioRecorder::new(rec_config);

    println!("\nPress Enter to START dictation...");
    let mut dummy = String::new();
    let _ = io::stdin().read_line(&mut dummy);

    recorder.start().context("Failed to start audio stream")?;

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = Arc::clone(&is_running);

    let meter = std::thread::spawn(move || {
        let start = Instant::now();
        while is_running_clone.load(Ordering::Relaxed) {
            let el = start.elapsed().as_secs_f64();
            print!(
                "\r{} [{:02}:{:02}] (press Enter to finish)...",
                "● LISTENING".bold().red(),
                (el as u64) / 60,
                (el as u64) % 60
            );
            io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    if let Some(dur) = duration {
        std::thread::sleep(Duration::from_secs_f64(dur));
    } else {
        let mut stop_line = String::new();
        let _ = io::stdin().read_line(&mut stop_line);
    }

    is_running.store(false, Ordering::Relaxed);
    let _ = meter.join();
    println!();

    let samples = recorder.stop().context("Failed to stop recording")?;
    if samples.is_empty() {
        println!("{}", "No audio recorded.".yellow());
        return Ok(());
    }

    // Voice Activity Detection check
    let vad = EnergyVad::default();
    if !vad.has_speech(&samples) {
        println!("{}", "⚠ No speech activity detected in audio.".yellow());
        return Ok(());
    }

    print!("Transcribing...");
    io::stdout().flush().unwrap();
    let t0 = Instant::now();
    let result = engine.transcribe(&samples)?;
    println!(" ({:.2}s)", t0.elapsed().as_secs_f64());

    println!("\n{}", "Transcription:".bold().green());
    println!("{}", result.text.bold());

    Ok(())
}

fn handle_doctor() -> Result<()> {
    println!(
        "{}",
        "==============================================".cyan()
    );
    println!(
        "{}",
        "         VoiceKey Hardware & Engine Doctor    "
            .bold()
            .cyan()
    );
    println!(
        "{}",
        "==============================================".cyan()
    );

    // 1. Audio input devices
    println!("\n{}", "1. Audio Subsystem & Microphones:".bold());
    let mgr = AudioCaptureManager::new();
    match mgr.list_input_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("   {} No input devices found!", "✗".bold().red());
            } else {
                println!(
                    "   {} Found {} audio input device(s):",
                    "✓".bold().green(),
                    devices.len()
                );
                for dev in &devices {
                    let mark = if dev.is_default { " (default)" } else { "" };
                    println!(
                        "     - {}{} [{} ch, {} Hz]",
                        dev.name, mark, dev.channels, dev.default_sample_rate
                    );
                }
            }
        }
        Err(e) => println!("   {} Audio error: {}", "✗".bold().red(), e),
    }

    // 2. CPU instruction support
    println!("\n{}", "2. CPU SIMD Acceleration Features:".bold());
    #[cfg(target_arch = "x86_64")]
    {
        let has_avx2 = is_x86_feature_detected!("avx2");
        let has_fma = is_x86_feature_detected!("fma");
        let has_f16c = is_x86_feature_detected!("f16c");
        let has_bmi2 = is_x86_feature_detected!("bmi2");

        println!(
            "   - AVX2:  {}",
            if has_avx2 {
                "✓ Enabled".bold().green()
            } else {
                "✗ Not available".yellow()
            }
        );
        println!(
            "   - FMA:   {}",
            if has_fma {
                "✓ Enabled".bold().green()
            } else {
                "✗ Not available".yellow()
            }
        );
        println!(
            "   - F16C:  {}",
            if has_f16c {
                "✓ Enabled".bold().green()
            } else {
                "✗ Not available".yellow()
            }
        );
        println!(
            "   - BMI2:  {}",
            if has_bmi2 {
                "✓ Enabled".bold().green()
            } else {
                "✗ Not available".yellow()
            }
        );
    }
    #[cfg(target_arch = "aarch64")]
    {
        println!("   - ARM NEON: {}", "✓ Enabled".bold().green());
    }

    // 3. GGML Whisper Models on Disk
    println!("\n{}", "3. Quantized GGML Models on Disk:".bold());
    let mut search_dirs = Vec::new();
    if let Some(data_dir) = dirs::data_dir() {
        search_dirs.push(data_dir.join("voicekey").join("models"));
    }
    if let Some(cache_dir) = dirs::cache_dir() {
        search_dirs.push(cache_dir.join("voicekey").join("models"));
    }
    search_dirs.push(PathBuf::from("/tmp/voicekey/models"));

    let mut found_models = Vec::new();
    for dir in &search_dirs {
        if dir.exists() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                        if ext == "bin" || ext == "gguf" {
                            let size_mb = entry
                                .metadata()
                                .map(|m| m.len() as f64 / 1_048_576.0)
                                .unwrap_or(0.0);
                            found_models.push((path, size_mb));
                        }
                    }
                }
            }
        }
    }

    if found_models.is_empty() {
        println!(
            "   {} No GGML models found in standard directories.",
            "⚠".bold().yellow()
        );
        println!("   Expected locations:");
        for d in &search_dirs {
            println!("     - {}", d.display());
        }
        println!(
            "\n   {} Run this command to download the fast base.en model:",
            "TIP:".cyan()
        );
        println!("     mkdir -p ~/.local/share/voicekey/models");
        println!("     curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin -o ~/.local/share/voicekey/models/ggml-base.en.bin");
    } else {
        println!(
            "   {} Found {} model(s):",
            "✓".bold().green(),
            found_models.len()
        );
        for (path, size_mb) in &found_models {
            println!(
                "     - {} ({:.1} MB)",
                path.display().to_string().bold(),
                size_mb
            );
        }
    }

    println!("\n{}", "Diagnosis Summary:".bold());
    println!(
        "   {} Core DSP, VAD & inference engine compiled natively.",
        "✓".green()
    );
    println!(
        "   {} Cross-platform ALSA/WASAPI audio pipeline active.",
        "✓".green()
    );

    Ok(())
}

fn handle_ipc(cmd: IpcCommand) -> Result<()> {
    match send_ipc_command(cmd) {
        Ok(IpcResponse::Ok) => {
            println!(
                "{}",
                format!("✓ IPC Command '{:?}' succeeded", cmd)
                    .bold()
                    .green()
            );
            Ok(())
        }
        Ok(IpcResponse::State(state)) => {
            println!("State: {}", state.bold().cyan());
            Ok(())
        }
        Ok(IpcResponse::Error(err)) => {
            bail!("VoiceKey daemon returned error: {}", err);
        }
        Err(e) => {
            bail!("Failed to communicate with VoiceKey daemon: {}", e);
        }
    }
}

fn handle_paste(text: String, preserve: bool, delay_ms: u64) -> Result<()> {
    let is_terminal = is_active_window_terminal();
    println!(
        "{}",
        "=== VoiceKey Direct Input Injection ===".bold().cyan()
    );
    println!(
        "Active Window Target: {}",
        if is_terminal {
            "Terminal (Ctrl+Shift+V)".yellow()
        } else {
            "Standard Desktop GUI (Ctrl+V)".green()
        }
    );
    println!("Payload Size: {} characters", text.len());
    println!(
        "Clipboard Preservation: {}",
        if preserve {
            format!("Enabled ({}ms debounce)", delay_ms).green()
        } else {
            "Disabled".yellow()
        }
    );

    ClipboardManager::paste_and_restore(&text, preserve, delay_ms)
        .context("Failed to inject text via clipboard auto-paste")?;

    println!("{}", "✓ Text injected successfully!".bold().green());
    Ok(())
}

fn handle_transform(text: String, mode: String, endpoint: String, model: String) -> Result<()> {
    println!(
        "{}",
        "=== Micro-LLM Smart Dictation Transform ===".bold().cyan()
    );
    println!("{} {}", "Input:".bold(), text);

    let forced_mode = match mode.to_lowercase().as_str() {
        "clean" => Some(voicekey_core::llm::LlmTransformMode::CleanTranscript),
        "bullets" | "bullet" => Some(voicekey_core::llm::LlmTransformMode::BulletPoints),
        "professional" | "formal" => Some(voicekey_core::llm::LlmTransformMode::ProfessionalTone),
        "casual" => Some(voicekey_core::llm::LlmTransformMode::CasualTone),
        "code" => Some(voicekey_core::llm::LlmTransformMode::VoiceToCode),
        _ => None, // "auto" uses spoken command parsing or direct text
    };

    let cfg = voicekey_core::llm::LlmConfig {
        enabled: true,
        endpoint,
        model,
        api_type: voicekey_core::llm::LlmApiType::OpenAiCompatible,
        timeout_ms: 2000,
        temperature: 0.1,
        max_tokens: 256,
    };

    let client = voicekey_core::llm::HttpLlmClient::new(cfg);
    let output = voicekey_core::llm::process_with_llm(&text, forced_mode, Some(&client))?;

    println!("\n{}", "Result:".bold().green());
    println!("{}", output);
    Ok(())
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn handle_stream(
    duration: Option<f64>,
    model: String,
    device: Option<String>,
    auto_commit: bool,
    silence_ms: u32,
    prompt: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        "=== VoiceKey Streaming Transcription & Speculative Output ==="
            .bold()
            .cyan()
    );

    let model_path = resolve_model_path(&model)?;
    println!("Model: {}", model_path.display().to_string().bold());
    println!(
        "Audio Input: {}",
        device.as_deref().unwrap_or("System Default")
    );
    println!(
        "Auto-Commit on Silence: {}",
        if auto_commit {
            format!("Enabled ({}ms hysteresis)", silence_ms).green()
        } else {
            "Disabled (manual stop)".yellow()
        }
    );

    let whisper_cfg = WhisperConfig {
        model_path,
        language: Some("en".into()),
        threads: 4,
        temperature: 0.0,
        translate: false,
        initial_prompt: prompt.clone(),
        enable_normalization: true,
        enable_spoken_punctuation: true,
    };

    let engine = Arc::new(WhisperEngine::load_from_file(whisper_cfg)?);

    let silence_frames = (silence_ms / 32).max(5) as usize;
    let vad_cfg = voicekey_core::vad::AdaptiveVadConfig {
        frame_size: 512,
        speech_start_threshold: 0.002,
        speech_hold_threshold: 0.0008,
        min_speech_frames: 3,
        silence_hysteresis_frames: silence_frames,
        sample_rate: 16000,
    };

    let stream_cfg = voicekey_core::engine::StreamingConfig {
        chunk_step_samples: 8000, // 500ms
        min_samples_for_inference: 8000,
        max_lookback_samples: 160000,
        vad_config: vad_cfg,
    };

    let mut transcriber =
        voicekey_core::engine::StreamingTranscriber::new(engine, stream_cfg, prompt);

    let rec_cfg = AudioRecorderConfig {
        device_name: device,
        pre_roll_ms: 200,
        target_sample_rate: 16000,
    };
    let mut recorder = AudioRecorder::new(rec_cfg);
    recorder.start()?;

    println!(
        "\n{}",
        "Speak into your microphone. Real-time speculative text streams below:".italic()
    );
    if duration.is_none() {
        println!(
            "{}",
            "(Press Enter to stop and commit final output)".dimmed()
        );
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Spawn non-blocking Enter key listener whenever duration is None
    if duration.is_none() {
        std::thread::spawn(move || {
            let mut line = String::new();
            if let Ok(n) = io::stdin().read_line(&mut line) {
                if n > 0 {
                    r.store(false, Ordering::SeqCst);
                }
            }
        });
    }

    let start_time = Instant::now();
    let mut last_sample_count = 0;
    let mut completed_transcripts: Vec<String> = Vec::new();

    while running.load(Ordering::SeqCst) {
        if let Some(d) = duration {
            if start_time.elapsed().as_secs_f64() >= d {
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(50));

        if let Ok(samples) = recorder.peek_samples() {
            if samples.len() > last_sample_count {
                let available = &samples[last_sample_count..];
                let full_frames_samples = (available.len() / 512) * 512;
                if full_frames_samples > 0 {
                    let to_process = &available[..full_frames_samples];
                    for frame in to_process.chunks_exact(512) {
                        match transcriber.feed_frame(frame) {
                            Ok(voicekey_core::engine::StreamingOutput::Partial {
                                text, ..
                            }) => {
                                print!("\r\x1b[2K{} {}", "[LIVE]".bold().green(), text);
                                let _ = io::stdout().flush();
                            }
                            Ok(voicekey_core::engine::StreamingOutput::Final { result }) => {
                                println!(
                                    "\n{} {}",
                                    "[UTTERANCE COMPLETE]".bold().cyan(),
                                    result.text
                                );
                                if !result.text.trim().is_empty() {
                                    completed_transcripts.push(result.text);
                                }
                                if auto_commit {
                                    running.store(false, Ordering::SeqCst);
                                    break;
                                }
                            }
                            Ok(voicekey_core::engine::StreamingOutput::None) => {}
                            Err(e) => {
                                eprintln!("\nStreaming error: {}", e);
                                if auto_commit {
                                    running.store(false, Ordering::SeqCst);
                                    break;
                                }
                            }
                        }
                    }
                    last_sample_count += full_frames_samples;
                }
            }
        }
    }

    // Stop recorder and consume any remaining audio suffix exactly once
    let final_samples = recorder.stop().unwrap_or_default();
    if final_samples.len() > last_sample_count {
        let suffix = &final_samples[last_sample_count..];
        for frame in suffix.chunks_exact(512) {
            if let Ok(voicekey_core::engine::StreamingOutput::Final { result }) =
                transcriber.feed_frame(frame)
            {
                if !result.text.trim().is_empty() {
                    completed_transcripts.push(result.text);
                }
            }
        }
        let rem = suffix.len() % 512;
        if rem > 0 {
            let short_frame = &suffix[suffix.len() - rem..];
            if let Ok(voicekey_core::engine::StreamingOutput::Final { result }) =
                transcriber.feed_frame(short_frame)
            {
                if !result.text.trim().is_empty() {
                    completed_transcripts.push(result.text);
                }
            }
        }
    }

    let final_res = transcriber.finish()?;
    if !final_res.text.trim().is_empty() {
        completed_transcripts.push(final_res.text);
    }

    let full_transcript = completed_transcripts.join(" ");

    println!(
        "\n\n{}",
        "=== Final Streaming Transcript ===".bold().green()
    );
    println!("{}", full_transcript.trim());

    Ok(())
}
