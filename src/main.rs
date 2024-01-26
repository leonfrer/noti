use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use config::Config;
use log::{debug, error, info};
use log4rs;
use notify::{Event, RecursiveMode, Watcher};
use notify::event::ModifyKind;
use notify::EventKind::Modify;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use reqwest::blocking::Client;
use reqwest::blocking::multipart;

const PATH: &str = "conf/Setting.toml";

pub fn refresh_settings() -> Config {
    info!("load settings");
    Config::builder()
        .add_source(config::File::with_name(PATH))
        .build()
        .unwrap()
}

fn main() {
    log4rs::init_file("conf/log4rs.yml", Default::default()).unwrap();

    let settings = refresh_settings();
    let (folder_tx, folder_rx) = channel();
    let mut debouncer = new_debouncer(Duration::from_secs(2), None, folder_tx).unwrap();

    let target_path: String = settings.get("target_path").unwrap();
    debouncer
        .watcher()
        .watch(Path::new(&target_path), RecursiveMode::Recursive)
        .unwrap();
    debouncer
        .cache()
        .add_root(Path::new(&target_path), RecursiveMode::Recursive);

    let client = Client::new();
    let url: String = settings.get("remote_base_url").unwrap();

    loop {
        match folder_rx.try_recv() {
            Ok(result) => {
                match result {
                    Ok(events) => events.iter().for_each(|event| {
                        if let DebouncedEvent {
                            event: Event { kind: Modify(ModifyKind::Any), paths, .. }, ..
                        } = event {
                            for path in paths {
                                if let Err(err) = handle_detected_file(&client, path, &url) {
                                    error!("file send error: {path:?}; {err:?}")
                                }
                            }
                        }
                    }),
                    Err(errors) => errors.iter().for_each(|error| error!("{error:?}")),
                }
            }
            Err(_) => {}
        }
    }
}

fn handle_detected_file(client: &Client, path: &PathBuf, url: &String) -> Result<(), Box<dyn Error>> {
    debug!("detect file modify {path:?}");
    // request
    if path.extension().unwrap_or_default() == "xls" {
        let form = multipart::Form::new()
            .file("file", path.clone().into_os_string().into_string().unwrap())?;

        let res = client.post(format!("{}{}", url, "/upload"))
            .multipart(form)
            .send()?;
        debug!("{:?}", res.text());

        info!("sent to server success: {path:?}")
    }
    Ok(())
}