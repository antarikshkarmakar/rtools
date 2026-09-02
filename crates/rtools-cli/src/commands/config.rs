use crate::ConfigCommands;
use rtools_core::AppConfig;

pub fn handle_config_command(cmd: ConfigCommands) -> anyhow::Result<()> {
    match cmd {
        ConfigCommands::Show => {
            let config = AppConfig::default();
            println!("Current configuration:");
            println!();
            println!("General:");
            println!("  Parallel jobs: {}", config.general.parallel_jobs);
            println!("  Temp dir: {}", config.general.temp_dir.display());
            println!("  Log level: {}", config.general.log_level);
            println!();
            println!("Image:");
            println!("  Default quality: {}", config.image.default_quality);
            println!("  Max dimension: {}", config.image.max_dimension);
            println!();
            println!("PDF:");
            println!("  OCR language: {}", config.pdf.ocr_language);
            println!("  OCR DPI: {}", config.pdf.ocr_dpi);
            println!();
            println!("AI:");
            println!("  Model dir: {}", config.ai.model_dir.display());
            println!("  Device: {:?}", config.ai.device);
            println!();
            println!("API:");
            println!("  Host: {}", config.api.host);
            println!("  Port: {}", config.api.port);
            Ok(())
        }

        ConfigCommands::Init { output } => {
            let config = AppConfig::default();
            config.save(&output)?;
            println!("✓ Configuration file created: {}", output.display());
            Ok(())
        }

        ConfigCommands::Validate {
            config: config_path,
        } => match AppConfig::load(Some(&config_path)) {
            Ok(_) => {
                println!("✓ Configuration file is valid: {}", config_path.display());
                Ok(())
            }
            Err(e) => {
                eprintln!("✗ Invalid configuration: {e}");
                std::process::exit(1);
            }
        },
    }
}
