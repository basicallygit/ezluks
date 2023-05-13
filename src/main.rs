use std::env::args;
use std::fs::create_dir_all;
use std::io::{stdin, stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::env;

#[link(name = "c")]
extern "C" {
    fn getuid() -> i32;
}

const FLUSH: fn() = || stdout().flush().unwrap();

#[inline]
fn has_cryptsetup() -> bool {
    Path::new("/usr/bin/cryptsetup").exists()
}

#[inline]
fn get_username() -> String {
    env::var("USER").expect("Failed to get username from $USER")
}

fn execute(command: Vec<&str>) {
    let output = Command::new(command[0])
        .args(&command[1..])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!("Command '{}' failed", command.join(" "));
        eprintln!("Aborting..");
        exit(1);
    }

    println!("{}", String::from_utf8_lossy(output.stdout.as_slice()));
}

fn print_usage() {
    println!("Usage: ezluks [command] [args]");
    println!("\nCommands:");
    println!("    open [block device e.g /dev/sda1] [mapper label e.g my_drive]");
    println!("    close [mapper label e.g my_drive]");
    println!("    format [block device e.g /dev/sda1]");
}

fn main() {
    let argv: Vec<String> = args().collect();
    let argc = argv.len();

    if !has_cryptsetup() {
        eprintln!("Could not find cryptsetup in your /usr/bin, aborting..");
        exit(1);
    }

    if unsafe { getuid() } != 0 {
        eprintln!("Not running as root, aborting..");
        exit(1);
    }

    match argc {
        3 => {
            if argv[1] == "close" {
                let mapper_label: &str = &argv[2];
                let mnt_path = format!("/run/media/{}/{}", get_username(), &mapper_label);

                println!("Unmounting {}", mnt_path);
                if !Path::new(&mnt_path).exists() {
                    eprintln!("Could not find path {}, aborting..", mnt_path);
                    exit(1);
                }
                execute(vec!["umount", &mnt_path]);
                println!("Using cryptsetup to close mapper label '{}'", mapper_label);
                execute(vec!["cryptsetup", "close", mapper_label]);
                println!("Successfully closed {}!", mapper_label);
            } else if argv[1] == "format" {
                let drive: &str = &argv[2];
                if !Path::new(drive).exists() {
                    eprintln!("Path {} does not exit, aborting..", drive);
                    exit(1);
                }
                println!(
                    "Are you SURE you want to luksFormat {}? THIS *WILL* WIPE ALL DATA ON IT!",
                    drive
                );
                print!("Enter the string yes in all capitals to continue: ");
                FLUSH();

                let mut input = String::new();
                stdin().read_line(&mut input).unwrap();

                if input.trim() != "YES" {
                    eprintln!("User did not confirm formatting of drive, aborting..");
                    exit(1);
                }
                println!("\ncryptsetup will also ask you to reconfirm this in a moment..");

                println!("Running 'cryptsetup luksFormat {}'..", drive);
                execute(vec!["cryptsetup", "luksFormat", drive]);
                print!("Give your new luks volume a mapper label: ");
                FLUSH();
                input.clear();

                stdin().read_line(&mut input).unwrap();
                let label_mnt = &format!("/run/media/{}/{}", get_username(), input.trim());
                let mapper_path = &format!("/dev/mapper/{}", input.trim());
                let label = &format!("{}", input.trim());

                println!("Running 'cryptsetup open {} {}'", drive, input.trim());
                execute(vec!["cryptsetup", "open", drive, input.trim()]);

                loop {
                    print!(
                        "What file system would you like this drive to have?\n(default = ext4): "
                    );
                    FLUSH();
                    input.clear();
                    stdin().read_line(&mut input).unwrap();
                    if input.trim().is_empty() {
                        input.clear();
                        input = String::from("ext4");
                    }
                    let mkfs_cmd = &format!("/usr/bin/mkfs.{}", input.trim());
                    if !Path::new(mkfs_cmd).exists() {
                        eprintln!("Could not find {}, please try again.", mkfs_cmd);
                    } else {
                        println!("Running '{} {}'", mkfs_cmd, mapper_path);
                        execute(vec![mkfs_cmd, mapper_path]);
                        break;
                    }
                }

                println!("Mounting /dev/mapper/{} to {}", label, label_mnt);
                if Path::new(label_mnt).exists() {
                    if !PathBuf::from(label_mnt).read_dir().unwrap().count() == 0 {
                        eprintln!("{} already exists and is not empty, closing with cryptsetup and aborting..", label_mnt);
                        execute(vec!["cryptsetup", "close", label]);
                        exit(1);
                    }
                } else {
                    create_dir_all(label_mnt).unwrap();
                }
                execute(vec!["mount", mapper_path, label_mnt]);
                println!(
                    "Successfully formatted your new drive and mounted to {}!",
                    label_mnt
                );
            } else {
                print_usage();
            }
        }
        4 => {
            if argv[1] == "open" {
                let drive = &argv[2];
                let label = &argv[3];
                let mnt_path = &format!("/run/media/{}/{}", get_username(), label);
                let mapper_label = &format!("/dev/mapper/{}", label);
                if !Path::new(drive).exists() {
                    eprintln!("Could not find path '{}', aborting..", drive);
                    exit(1);
                }
                println!("Running 'cryptsetup open {} {}'", drive, label);
                execute(vec!["cryptsetup", "open", drive, label]);
                println!("Mounting {} to {}", mapper_label, mnt_path);
                if Path::new(mnt_path).exists() {
                    if !PathBuf::from(mnt_path).read_dir().unwrap().count() == 0 {
                        eprintln!("{} already exists and is not empty, closing with cryptsetup and aborting..", mnt_path);
                        execute(vec!["cryptsetup", "close", label]);
                        exit(1);
                    }
                } else {
                    create_dir_all(mnt_path).unwrap();
                }
                execute(vec!["mount", mapper_label, mnt_path]);
                println!("Successfully decrypted and mounted drive to {}!", mnt_path);
            } else {
                print_usage();
            }
        }
        _ => print_usage(),
    }
}
