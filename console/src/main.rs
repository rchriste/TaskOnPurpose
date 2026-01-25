#![warn(unused_crate_dependencies)]

pub(crate) mod base_data;
pub(crate) mod calculated_data;
pub(crate) mod data_storage;
pub(crate) mod display;
pub(crate) mod menu;
pub(crate) mod new_event;
pub(crate) mod new_item;
pub(crate) mod new_mode;
pub(crate) mod new_time_spent;
mod node;
pub(crate) mod systems;

use std::{
    env,
    io::{self, Write},
    process,
    time::{Duration, SystemTime},
};

use better_term::Style;
use crossterm::terminal::{Clear, ClearType};
use icy_sixel::{EncodeOptions, QuantizeMethod, sixel_encode};
use image::{
    imageops::{self, FilterType},
    load_from_memory,
};
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet, Styled};
use mimalloc::MiMalloc;

use tokio::sync::mpsc;

use crate::{
    data_storage::surrealdb_layer::data_layer_commands::{
        self, CopyDestinationBehavior, SurrealAuthConfig, data_storage_start_and_run,
    },
    menu::inquire::do_now_list_menu::present_normal_do_now_list_menu,
};

#[derive(Debug, Clone)]
struct CliSurrealConfig {
    endpoint: String,
    namespace: String,
    username: String,
    auth_username: Option<String>,
    auth_password: Option<String>,
    auth_level: Option<String>,
    initialize_from_endpoint: Option<String>,
    initialize_from_namespace: Option<String>,
    initialize_from_database: Option<String>,
    initialize_from_copy_behavior: CopyDestinationBehavior,
}

fn print_help_and_exit() -> ! {
    eprintln!(
        r#"Task On Purpose

Usage:
  taskonpurpose [inmemorydb]
    [--surreal-endpoint <endpoint>]
    [--namespace <ns>]
    [--username <user>]
    [--initialize-from-database <db> [--initialize-from-endpoint <endpoint>] [--initialize-from-namespace <ns>]]
    [--surreal-auth-username <user> --surreal-auth-password <pass> [--surreal-auth-level <root|ns|db>]]

Options:
  --surreal-endpoint, -e        SurrealDB connection string/endpoint (e.g. mem://, file://..., ws://...)
  --namespace, -n               SurrealDB namespace (default: TaskOnPurpose)
  --username, --user, -u        Username to use as the SurrealDB *database name* (default: OS user)
  --initialize-from-database    This option makes it possible to state the initialize-from username, as the username is how the database name is set. One-shot: copy all data from a source SurrealDB database into the destination database, but only if the destination is empty (unless used together with --force, which first deletes all existing data in the destination)
  --initialize-from-endpoint    Source endpoint for --initialize-from-database (default: same as destination endpoint)
  --initialize-from-namespace   Source namespace for --initialize-from-database (default: same as destination namespace)
  --force                       Use with --initialize-from-database: when the destination database already contains data, delete it before copying (overrides the 'destination must be empty' requirement; use with extreme caution!)
  --surreal-auth-username       SurrealDB login username (optional; used for remote auth)
  --surreal-auth-password       SurrealDB login password (optional; used for remote auth)
  --surreal-auth-level          SurrealDB auth level: root | ns | db (default: root)
  --help, -h                    Show this help

Notes:
  - The SurrealDB database name is derived from the provided username (this replaces the previous hardcoded \"Russ\").
  - On startup, if namespace \"TaskOnPurpose\" is empty but legacy namespace \"OnPurpose\" has data, the data is copied into \"TaskOnPurpose\".
  - If connecting to a remote SurrealDB with IAM enabled, you likely need to pass `--surreal-auth-username/--surreal-auth-password`.
"#
    );
    process::exit(0);
}

fn default_os_username() -> String {
    env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "default".to_string())
}

fn parse_cli(args: &[String]) -> Result<CliSurrealConfig, String> {
    // Back-compat: `inmemorydb` positional arg still works.
    //Default location can be overridden via CLI args
    let mut endpoint = "file://c:/.on_purpose.db".to_string();

    let mut namespace = "TaskOnPurpose".to_string();
    let mut username = default_os_username();
    let mut auth_username: Option<String> = None;
    let mut auth_password: Option<String> = None;
    let mut auth_level: Option<String> = None;
    let mut initialize_from_endpoint: Option<String> = None;
    let mut initialize_from_namespace: Option<String> = None;
    let mut initialize_from_database: Option<String> = None;
    let mut initialize_from_copy_behavior = CopyDestinationBehavior::ErrorIfNotEmpty;

    let mut i = 1usize;
    while i < args.len() {
        if args[i] == "inmemorydb" {
            endpoint = "mem://".to_string();
            i += 1;
            continue;
        }
        match args[i].as_str() {
            "--help" | "-h" => print_help_and_exit(),
            "-username" | "-user" => {
                return Err(format!(
                    "Unrecognized option '{}'. Did you mean '--username <user>' (or '-u <user>')?",
                    args[i]
                ));
            }
            "--surreal-endpoint" | "--endpoint" | "-e" => {
                i += 1;
                endpoint = args
                    .get(i)
                    .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                    .to_string();
            }
            "--namespace" | "--ns" | "-n" => {
                i += 1;
                namespace = args
                    .get(i)
                    .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                    .to_string();
            }
            "--username" | "--user" | "-u" => {
                i += 1;
                username = args
                    .get(i)
                    .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                    .to_string();
            }
            "--surreal-auth-username" | "--auth-username" | "--surreal-user" => {
                i += 1;
                auth_username = Some(
                    args.get(i)
                        .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                        .to_string(),
                );
            }
            "--surreal-auth-password" | "--auth-password" | "--surreal-pass" => {
                i += 1;
                auth_password = Some(
                    args.get(i)
                        .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                        .to_string(),
                );
            }
            "--surreal-auth-level" | "--auth-level" => {
                i += 1;
                auth_level = Some(
                    args.get(i)
                        .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                        .to_string(),
                );
            }
            "--initialize-from-endpoint" => {
                i += 1;
                initialize_from_endpoint = Some(
                    args.get(i)
                        .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                        .to_string(),
                );
            }
            "--initialize-from-namespace" => {
                i += 1;
                initialize_from_namespace = Some(
                    args.get(i)
                        .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                        .to_string(),
                );
            }
            "--initialize-from-database" | "--initialize-from-db" => {
                i += 1;
                initialize_from_database = Some(
                    args.get(i)
                        .ok_or_else(|| format!("Missing value for {}", args[i - 1]))?
                        .to_string(),
                );
            }
            "--force" => {
                initialize_from_copy_behavior = CopyDestinationBehavior::ForceDeleteExisting;
            }
            _ => {
                return Err(format!("Unknown argument '{}'. Try --help.", args[i]));
            }
        }
        i += 1;
    }

    if initialize_from_database.is_none()
        && (initialize_from_endpoint.is_some() || initialize_from_namespace.is_some())
    {
        return Err(
            "--initialize-from-endpoint/--initialize-from-namespace require --initialize-from-database"
                .to_string(),
        );
    }

    Ok(CliSurrealConfig {
        endpoint,
        namespace,
        username,
        auth_username,
        auth_password,
        auth_level,
        initialize_from_endpoint,
        initialize_from_namespace,
        initialize_from_database,
        initialize_from_copy_behavior,
    })
}

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Keep Inquire's existing theme for prompt/help text, but make the currently highlighted
    // option line (and its prefix) orange + bold for better visibility.
    //
    // Note: We use ANSI 256-color code 208 (orange) for broad terminal support.
    let render_config: RenderConfig<'static> = RenderConfig::default()
        .with_highlighted_option_prefix(
            Styled::new(">")
                .with_fg(Color::AnsiValue(208))
                .with_attr(Attributes::BOLD),
        )
        .with_selected_option(Some(
            StyleSheet::new()
                .with_fg(Color::AnsiValue(208))
                .with_attr(Attributes::BOLD),
        ));
    inquire::set_global_render_config(render_config);

    println!("{}", Clear(ClearType::All));
    print_hourglass_logo().unwrap_or_else(|err| eprintln!("Unable to display logo (sixel): {err}"));

    const CARGO_PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

    println!(
        "{}Welcome to 🕜 Task On Purpose 🕜{}",
        Style::default().bold(),
        Style::default()
    );
    println!(
        "{}Version {}{}",
        Style::default().italic(),
        CARGO_PKG_VERSION.unwrap_or("UNKNOWN"),
        Style::default()
    );
    println!();

    let commands_in_flight_limit = 20;
    let (send_to_data_storage_layer_tx, have_data_storage_layer_use_to_receive_rx) =
        mpsc::channel(commands_in_flight_limit);

    let args: Vec<String> = env::args().collect();
    let surreal_cli = match parse_cli(&args) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Error parsing CLI args: {err}\nTry --help.");
            std::process::exit(2);
        }
    };

    let auth = match (
        surreal_cli.auth_username.clone(),
        surreal_cli.auth_password.clone(),
    ) {
        (Some(user), Some(pass)) => Some(SurrealAuthConfig {
            username: user,
            password: pass,
            level: surreal_cli.auth_level.clone(),
        }),
        (None, None) => None,
        _ => {
            eprintln!(
                "If providing SurrealDB auth, you must provide both --surreal-auth-username and --surreal-auth-password."
            );
            process::exit(2);
        }
    };

    // One-shot: copy data from a source DB into the destination DB (destination must be empty).
    if let Some(initialize_from_db) = surreal_cli.initialize_from_database.clone() {
        let source = data_layer_commands::SurrealDbConnectionConfig {
            endpoint: surreal_cli
                .initialize_from_endpoint
                .clone()
                .unwrap_or_else(|| surreal_cli.endpoint.clone()),
            namespace: surreal_cli
                .initialize_from_namespace
                .clone()
                .unwrap_or_else(|| surreal_cli.namespace.clone()),
            database: initialize_from_db,
            auth: auth.clone(),
        };
        let destination = data_layer_commands::SurrealDbConnectionConfig {
            endpoint: surreal_cli.endpoint.clone(),
            namespace: surreal_cli.namespace.clone(),
            database: surreal_cli.username.clone(),
            auth: auth.clone(),
        };

        eprintln!(
            "Copying SurrealDB data: src(endpoint='{}' ns='{}' db='{}') -> dest(endpoint='{}' ns='{}' db='{}')",
            source.endpoint,
            source.namespace,
            source.database,
            destination.endpoint,
            destination.namespace,
            destination.database
        );

        if let Err(err) = data_layer_commands::copy_database_if_destination_empty(
            source,
            destination,
            surreal_cli.initialize_from_copy_behavior,
        )
        .await
        {
            eprintln!("Copy failed: {err}");
            process::exit(2);
        }

        eprintln!("Copy complete.");
        return Ok(());
    }

    let data_storage_join_handle = tokio::spawn(async move {
        data_storage_start_and_run(
            have_data_storage_layer_use_to_receive_rx,
            crate::data_storage::surrealdb_layer::data_layer_commands::SurrealDbConnectionConfig {
                endpoint: surreal_cli.endpoint,
                namespace: surreal_cli.namespace,
                database: surreal_cli.username,
                auth,
            },
        )
        .await
    });
    //If the current executable is more than 3 months old print a message that there is probably a newer version available
    let exe_path = env::current_exe().unwrap();
    let exe_metadata = exe_path.metadata().unwrap();
    let exe_modified = exe_metadata.modified().unwrap();
    let now = SystemTime::now();
    let three_months = Duration::from_secs(60 * 60 * 24 * 30 * 3);
    if now.duration_since(exe_modified).unwrap() > three_months {
        println!(
            "This version of On Purpose is more than 3 months old. You may want to check for a newer version at https://github.com/rchriste/OnPurpose/releases"
        );
    }

    loop {
        match present_normal_do_now_list_menu(&send_to_data_storage_layer_tx).await {
            Result::Ok(..) => (),
            Result::Err(..) => break,
        };

        if data_storage_join_handle.is_finished() {
            println!("Data Storage Layer closed early, unexpectedly");
        }
    }

    drop(send_to_data_storage_layer_tx);

    print!("Waiting for data storage layer to exit...");
    data_storage_join_handle.await.unwrap();
    println!("Done");

    Ok(())
}

/// Prints the OnPurpose hourglass logo to stdout as a sixel-encoded image.
///
/// This function loads the embedded PNG logo, resizes it to fit within terminal dimensions,
/// and outputs it using the sixel graphics format. The terminal must support sixel graphics
/// for the image to display correctly (e.g., Windows Terminal with Atlas rendering engine,
/// or terminals like mlterm, wezterm, or xterm with sixel support).
///
/// # Errors
///
/// Returns an error if:
/// - The embedded logo image cannot be loaded or decoded
/// - Image resizing or sixel encoding fails
/// - Writing to stdout fails
fn print_hourglass_logo() -> Result<(), Box<dyn std::error::Error>> {
    let canvas = load_from_memory(include_bytes!("logo/hourglass_logo.png"))?.to_rgba8();

    // Keep the original aspect; only scale down if needed.
    let resized = resize_to_fit(canvas, 130, 260);

    let encode_opts = EncodeOptions {
        max_colors: 256,
        diffusion: 0.5, //Reduced dithering, less noise, good for graphics Higher values produce smoother gradients but may introduce noise. Lower values preserve sharp edges but may show color banding. Values are clamped to the range 0.0-1.0.
        quantize_method: QuantizeMethod::Wu,
    };
    let sixel = sixel_encode(
        resized.as_raw(),
        resized.width() as usize,
        resized.height() as usize,
        &encode_opts,
    )?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(sixel.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

/// Resizes an image to fit within specified dimensions while preserving aspect ratio.
///
/// This function only scales down images that exceed the specified dimensions - it will never
/// scale up a smaller image. If the image is already smaller than or equal to the maximum
/// dimensions, it returns a clone of the original image unchanged.
///
/// # Arguments
///
/// * `img` - The source image to resize
/// * `max_width` - Maximum width constraint in pixels
/// * `max_height` - Maximum height constraint in pixels
///
/// # Returns
///
/// A new `RgbaImage` that fits within the specified dimensions while maintaining the original
/// aspect ratio. If no resizing is needed, returns the input image.
fn resize_to_fit(img: image::RgbaImage, max_width: u32, max_height: u32) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    let scale_w = max_width as f32 / w as f32;
    let scale_h = max_height as f32 / h as f32;
    let scale = scale_w.min(scale_h).min(1.0);

    if scale >= 1.0 {
        img
    } else {
        let new_w = (w as f32 * scale).round().max(1.0) as u32;
        let new_h = (h as f32 * scale).round().max(1.0) as u32;
        imageops::resize(&img, new_w, new_h, FilterType::Lanczos3)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cli;

    #[test]
    fn parse_cli_rejects_single_dash_username_flag() {
        let args = vec![
            "taskonpurpose".to_string(),
            "-username".to_string(),
            "nash".to_string(),
        ];

        // This should be rejected; the correct flag is `--username` or `-u`.
        // (Added as a regression test; it should fail before we implement the behavior.)
        assert!(parse_cli(&args).is_err());
    }

    #[test]
    fn parse_cli_errors_on_unknown_argument() {
        let args = vec!["taskonpurpose".to_string(), "--unexpected".to_string()];
        let err = parse_cli(&args).expect_err("Should fail for unknown argument");
        assert!(err.contains("Unknown argument"));
    }
}
