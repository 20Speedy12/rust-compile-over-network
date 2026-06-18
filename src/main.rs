#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use ssh::*;
use tokio::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::fs::File;
use std::path::{Path};
use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::env;

#[derive(Serialize, Deserialize)]
struct Config {
    target_ip: String,
    project_folder: String,
    arch: String,
    bit:String,
    target_os: String,
    is_release: bool,
}

#[derive(Debug, Deserialize)]
struct Cargoparses {
    package: Packageparses,
}
#[derive(Debug, Deserialize)]
struct Packageparses {
    name: String,
}



type SharedLog = Arc<RwLock<Vec<String>>>;

#[tokio::main]
async fn main() -> eframe::Result {
    //println!("{}", Path::new("/etc/hosts").exists());

    let shared_log: SharedLog = Arc::new(RwLock::new(Vec::new()));

    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([500.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "compile over network",
        options,
        Box::new({
            let gui_log = Arc::clone(&shared_log);
            move |_cc| {
                Ok(Box::new(MyApp::new(gui_log)))
            }
        }),
    )
}

struct MyApp {
    ip: String,
    arch: String,
    bit: String,
    compiling: bool,
    shared_log: SharedLog,
    logs: Vec<String>,
    os: String,
    picked_path: Option<String>,
    final_path: String,
    picked: bool,
    target_location: String,
    is_release: bool,
    saved: bool,
    loaded: bool,
}

impl MyApp {

    fn new(shared_log: SharedLog) -> Self {
        Self {
            ip: "user@server".to_owned(),
            arch: "x86_".to_owned(),
            bit: "64-".to_owned(),
            compiling: false,
            shared_log,
            logs: vec!["ssh logs show here".to_owned()],
            os: "windows-gnu".to_owned(),
            picked_path: Some("".to_owned()),
            final_path: "".to_owned(),
            picked:false,
            target_location: "/tmp/extraction".to_owned(),
            is_release: false,
            saved: false,
            loaded: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.loaded{
            let exepath = std::env::current_exe().unwrap().to_string_lossy().into_owned();
            let finalpathlocal = exepath + "/save.toml";
            let savetoml = Path::new(&(finalpathlocal));
            if savetoml.exists(){
                let savecontents = std::fs::read_to_string(savetoml).expect("Failed to read config file");
                let configurations: Config = toml::from_str(&savecontents).expect("loading the save failed");
                self.arch = configurations.arch;
                self.bit = configurations.bit;
                self.ip = configurations.target_ip;
                self.final_path = configurations.project_folder;
                self.is_release = configurations.is_release;
                self.os = configurations.target_os;
                self.loaded = true;
            }

        }



        if let Ok(read_guard) = self.shared_log.try_read() {
            if read_guard.len() != self.logs.len() {
                self.logs = read_guard.clone();
        }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let name_label = ui.label("Target machine IP & User: ");
                ui.text_edit_singleline(&mut self.ip)
                    .labelled_by(name_label.id);
            });

            ui.horizontal(|ui| {
                if ui.button("x86").clicked() { self.arch = "x86_".to_string(); }
                if ui.button("arm").clicked() { self.arch = "aarch".to_string(); }
            });
            ui.horizontal(|ui| {
                if ui.button("64").clicked() { self.bit = "64-".to_owned(); }
                if ui.button("32").clicked() { self.bit = "32-".to_owned(); }
            });
            ui.horizontal(|ui| {
                if ui.button("Windows").clicked() { self.os = "pc-windows-gnu".to_string(); }
                if ui.button("Linux").clicked() { self.os = "unknown-linux-gnu".to_string(); }
            });
            ui.horizontal(|ui| {
                if ui.button("Debug build").clicked() { self.is_release = false; }
                if ui.button("Release build").clicked() { self.is_release = true; }
            });
            ui.horizontal(|ui| {
                let name_label = ui.label("Where to save to on target machine: ");
                ui.text_edit_singleline(&mut self.target_location)
                    .labelled_by(name_label.id);
            });

            ui.label(format!("machine {}, architecture {}, {} bit, target os {}, is release build, {}", self.ip, self.arch, self.bit, self.os, self.is_release));

            if ui.button("project folder…").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                self.picked_path = Some(path.display().to_string());
                self.final_path = self.picked_path.as_mut().unwrap().to_string();
                if Path::new(&(self.final_path.to_string() + &"/src/main.rs".to_string())).exists(){
                    println!("{}", self.final_path);
                    println!("{}", &(self.final_path.to_string() + &"/src/main.rs".to_string()));
                    self.picked = true;
                } else {
                    println!("bad folder");
                    self.picked_path = Some("".to_owned());
                    self.final_path = "".to_owned();
                }

            }

            if let Some(picked_path) = &self.picked_path {
                ui.horizontal(|ui| {
                    ui.label("Picked folder:");
                    ui.monospace(picked_path);
                });
            }

            if !self.compiling && self.picked {
                if ui.button("compile").clicked() {
                    if !self.saved{
                        let savefile = Config{
                            target_ip: self.ip.clone(),
                            arch: self.arch.clone(),
                            project_folder: self.final_path.clone(),
                            bit: self.bit.clone(),
                            target_os: self.os.clone(),
                            is_release: self.is_release.clone()
                        };
                        match toml::to_string(&savefile) {
                            Ok(actual_string) => {
                                if let Err(e) = std::fs::write("save.toml", actual_string) {
                                } else {
                                }
                            }
                            Err(e) => {
                            }
                        }

                        self.saved = true;
                    }
                    let host = self.ip.clone();
                    let arch = self.arch.clone();
                    let bit = self.bit.clone();
                    let os = self.os.clone();
                    let path = self.final_path.clone();
                    let is_release = self.is_release.clone();
                    let task_log = Arc::clone(&self.shared_log);
                    let ctx_clone = ctx.clone();
                    let target_location = self.target_location.clone();

                    tokio::spawn(async move {
                        let _ = actuallycomp(host, arch, bit, os, path, target_location, is_release, task_log, ctx_clone).await;
                    });

                    self.compiling = true;
                }
            }

            if self.compiling {

                ui.label("compiling!");

                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        egui::Frame::canvas(ui.style()).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for log in &self.logs {
                                ui.monospace(log);
                            }
                        });
                    });
            }
            ui.label("do not use ~ to indicate home directory and saves to ~/sshnetcomp/ and the executable is whatever your projects name is");
        });
    }
}



async fn actuallycomp(host: String, mut arch: String, mut bit: String, mut os: String, profold: String, target_location: String, is_release: bool,  shared_log: SharedLog, ctx: egui::Context ) -> Result<()> {
    let profoldclone = profold.clone() + "/Cargo.toml";
    let cargotomlpath = Path::new(&profoldclone);
    let tomlcontents = fs::read_to_string(cargotomlpath).await?;
    let tomlconfig: Cargoparses = toml::from_str(&tomlcontents)?;
    let projname = tomlconfig.package.name;
    let releaseflag;
    let is_release_folder;
    if is_release{
        releaseflag = "--release";
        is_release_folder = "/release/";
    }else{
        releaseflag = "";
        is_release_folder = "/debug/";
    }

    let host1 = host.to_owned();
    if arch == "x86_".to_owned() && bit == "32-".to_owned(){
        bit = "i686-".to_string();
            arch = "".to_string();
    }
    if arch == "aarch".to_owned() && os == "pc-windows-gnu".to_owned(){
        os = "pc-windows-gnullvm".to_owned()
    }
    if arch == "aarch".to_owned() && os == "unknown-linux-gnu".to_owned(){
        os = "unknown-linux-gnu".to_owned()
    }
    let exepath = target_location.clone() + "/target/" + &arch + &bit + &os + &is_release_folder + &projname;
    println!("{}", exepath);
    let comman = "mkdir -pv ".to_owned() + &target_location + " && cd " + &target_location + " && tar -xvf /tmp/extraction/gobstap.tar.gz -C " + &target_location + " && rm /tmp/extraction/gobstap.tar.gz && $HOME/.cargo/bin/cargo build " + &releaseflag + " --target " + &arch + &bit + &os;

    let tar_gz = File::create("/tmp/gobstap.tar.gz")?;
    let enc = GzEncoder::new(tar_gz, Compression::none());
    let mut tar = Builder::new(enc);
    //tar.append_dir_all(".", &profold);
    //tar.into_inner()?.finish();
    let root_path = Path::new(&profold);
    append_dir_filtered(&mut tar, root_path, root_path)?;
    let enc = tar.into_inner()?;
    enc.finish()?;
    /*
     i686-unknown-linux-gnu = 32 bit - works
     x86_64-unknown-linux-gnu = 64 bit - works
     aarch64-linux-gnu-gcc = arm 64 - works
     aarch32-linux-gnu-gcc = arm 32 - works
     x86_64-pc-windows-gnu = windows 64 bit - works
     i686-pc-windows-gnu = windows 32 bit - works
     aarch64-pc-windows-gnullvm = windows 64 arm - works
     aarch32-pc-windows-gnullvm = windows 32 arm - works
     */
    let command = comman.to_string();
    println!("{}", command);

    let _ = tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("spawned");
        let mut session = Session::new().unwrap();
        session.set_host(&host1).unwrap();
        session.parse_config(None).unwrap();
        session.connect().unwrap();

        if session.is_server_known().is_ok() {
            println!("serverknown");
            session.userauth_publickey_auto(Some("~/.ssh/id_rsa")).unwrap();

                 let mut scp=session.scp_new(RECURSIVE|WRITE,"/tmp").unwrap();
                 scp.init().unwrap();
                 scp.push_directory("extraction",0o755).unwrap();
                 println!("made directory");
                 let mut local_file = File::open("/tmp/gobstap.tar.gz")?;
                 let metadata = std::fs::metadata("/tmp/gobstap.tar.gz")?;
                 let file_size = metadata.len();
                 scp.push_file("gobstap.tar.gz", file_size as usize, 0o644).unwrap();
                 let mut buffer = vec![0; 16384];
                     loop {
                         //let mut local_file = File::open("/tmp/gobstap.tar.gz")?;
                         println!("doing something");
                         let bytes_read = local_file.read(&mut buffer)?;
                         if bytes_read == 0 {
                             break;
                         }
                         let _ = scp.write(&buffer[..bytes_read]);
                     }
                //scp.push_file("extraction.tar.gz",buffer.len(),0o644).unwrap();
                 //scp.write(buffer).unwrap();
            }
            {

            let mut s = session.channel_new().unwrap();
            s.open_session().unwrap();
            println!("extracting");
            let full_remote_cmd = format!(
                //"bash -c 'tar -xvf /tmp/extraction/gobstap.tar.gz -C /tmp/extraction && cd /tmp/extraction && {}' 2>&1",
                "bash -c '{}' 2>&1",
                command
                //let comman = "mkdir -pv ".to_owned() + &target_location + " && cd " + &target_location + " && tar -xvf /tmp/extraction/gobstap.tar.gz -C " + &target_location + " && $HOME/.cargo/bin/cargo build --target " + &arch + &bit + &os;
                //mkdir -pv /compilehere && cd /compilehere && tar -xvf /tmp/extraction/gobstap.tar.gz -C /compilehere  && $HOME/.cargo/bin/cargo build --target x86_unknown_linux_gnu or whatever
            );
            s.request_exec(full_remote_cmd.as_bytes()).unwrap();

            //s.request_exec(b"ls -l").unwrap();

            s.send_eof().unwrap();



            let mut reader = BufReader::new(s.stdout());
            let mut raw_line = String::new();

            while let Ok(bytes_read) = reader.read_line(&mut raw_line) {
                if bytes_read == 0 { break; }
                let clean_line = raw_line.trim_end().to_string();

                {
                    let mut write_guard = shared_log.blocking_write();
                    write_guard.push(clean_line);
                }
                ctx.request_repaint();
                raw_line.clear();

            }
            drop(reader);
            s.send_eof().unwrap();
            s.close();


            }
            println!("broke out of that bit");
            let mut executbale: Vec<u8> = vec![];
            let mut s = session.channel_new().unwrap();
            s.open_session().unwrap();
            s.request_exec(format!("cat {}", exepath).as_bytes()).unwrap();
            s.send_eof().unwrap();
            s.stdout().read_to_end(&mut executbale).unwrap();
            println!("read {} bytes", executbale.len());

            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let ondevicepath = format!("{}/sshnetcomp/{}", home, projname);
            std::fs::create_dir_all(format!("{}/sshnetcomp", home))?;
            std::fs::write(&ondevicepath, &executbale)?;
            println!("saved to {}", ondevicepath);

            Ok(())
        }

    ).await.unwrap();
    let _ = fs::remove_file("/tmp/gobstap.tar.gz");
Ok(())
}

fn append_dir_filtered<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    base_path: &Path,
    current_path: &Path,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();


        if file_name == "target" || file_name == ".git" {
            continue;
        }


        let rel_path = path.strip_prefix(base_path).unwrap();

        if path.is_dir() {

            builder.append_dir(rel_path, &path)?;


            append_dir_filtered(builder, base_path, &path)?;
        } else {

            builder.append_path_with_name(&path, rel_path)?;
        }
    }
    Ok(())
}
/* planning
get ssh in here -- done
select folder -- done
copy over to server in a temporary placement -- done
copy finished file back -- done
show output -- done
make a saving (using json?, no toml) -- not done
make sure that server has propper compiler -- not done
make progress bar -- not done - probably won't do
make it not just ubuntu/debian compatible -- not done
make it use the same folder so it doesn't spend insanely long amounts of time compiling -- done
add the --release flag -- done
*/
