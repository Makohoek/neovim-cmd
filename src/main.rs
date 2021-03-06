extern crate clap;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate neovim_lib;

use clap::{App, SubCommand};
use neovim_lib::{Handler, Neovim, NeovimApi, Session, Value};
use std::env;
use std::sync::mpsc;

pub enum BufferEvent {
    Delete,
}

pub struct BufferEventHandler(pub mpsc::Sender<BufferEvent>);

impl BufferEventHandler {
    fn parse_buf_detach_event(&mut self, _args: &Vec<Value>) -> Result<BufferEvent, String> {
        Ok(BufferEvent::Delete)
    }
}

impl Handler for BufferEventHandler {
    fn handle_notify(&mut self, name: &str, args: Vec<Value>) {
        debug!("event: {}", name);
        match name {
            "nvim_buf_detach_event" => {
                if let Ok(event) = self.parse_buf_detach_event(&args) {
                    debug!("got detach event!");
                    if let Err(_e) = self.0.send(event) {
                        error!("Error sending buf_detach_event");
                    }
                }
            }
            "nvim_buf_changedtick_event" => {}
            _ => {}
        }
    }

    fn handle_request(&mut self, _name: &str, _args: Vec<Value>) -> Result<Value, Value> {
        Err(Value::from("not implemented"))
    }
}

fn main() {
    env_logger::init();

    // https://rust-lang-nursery.github.io/rust-cookbook/app.html#ex-clap-basic
    let matches = App::new("neovim-cmd")
        .version("0.1.0")
        .author("Mattijs Korpershoek <mattijs.korpershoek@gmail.com>")
        .about("Send commands to neovim from the :terminal")
        .subcommand(
            SubCommand::with_name("edit")
                .about("Edits file in existing neovim instance")
                .arg_from_usage("<file> 'File to edit'")
                .arg_from_usage("--wait 'Wait for buffer to be deleted'"),
        )
        .subcommand(
            SubCommand::with_name("cd")
                .about("Changes neovim's :pwd")
                .arg_from_usage("[directory] 'Directory to :tchdir'"),
        )
        .subcommand(
            SubCommand::with_name("rename")
                .about("Renames current :terminal")
                .arg_from_usage("<name> 'Rename current :terminal to <name>'"),
        )
        .get_matches();

    // first, check if we are within neovim's terminal (if neovim is running)
    let address = match env::vars().find(|&(ref key, ref _value)| key == "NVIM_LISTEN_ADDRESS") {
        // option.0 is the key (env variable name) option.1 is the value (env variable value)
        Some(option) => option.1,
        None => {
            error!("This only works from within a neovim terminal");
            return;
        }
    };

    // create a session and start it
    debug!("listening address {}", address);

    let session = Session::new_unix_socket(address).unwrap();

    if let Some(matches) = matches.subcommand_matches("edit") {
        let filename = matches.value_of("file").unwrap();
        let filename = filename.to_string();
        if matches.is_present("wait") {
            edit_wait(session, filename);
        } else {
            edit(session, filename);
        }
        return;
    }
    if let Some(matches) = matches.subcommand_matches("cd") {
        let directory = match matches.value_of("directory") {
            Some(d) => d.to_string(),
            None => String::from(""),
        };
        tchdir(session, directory);
        return;
    }
    if let Some(matches) = matches.subcommand_matches("rename") {
        let bufname = matches.value_of("name").unwrap();
        let bufname = bufname.to_string();
        term_rename(session, bufname);
        return;
    }
}

fn tchdir(mut session: Session, directory: String) {
    let command = String::from("tchdir");
    let command = command + " ";
    let command = command + &directory;

    session.start_event_loop();

    // create the nvim instance
    let mut nvim = Neovim::new(session);

    // send some commands
    nvim.command(&command).unwrap();
}

fn term_rename(mut session: Session, name: String) {
    let command = String::from("file");
    let command = command + " term://";
    let command = command + &name;

    session.start_event_loop();

    // create the nvim instance
    let mut nvim = Neovim::new(session);

    // send some commands
    nvim.command(&command).unwrap();
}

fn edit(mut session: Session, filename: String) {
    // build the command to send to neovim
    let command = String::from("edit");
    let command = command + " ";
    let command = command + &filename;

    session.start_event_loop();

    // create the nvim instance
    let mut nvim = Neovim::new(session);

    // send some commands
    nvim.command(&command).unwrap();
}

fn edit_wait(mut session: Session, filename: String) {
    // build the command to send to neovim
    let command = String::from("edit");
    let command = command + " ";
    let command = command + &filename;

    let (sender, receiver) = mpsc::channel();
    session.start_event_loop_handler(BufferEventHandler(sender));

    // create the nvim instance
    let mut nvim = Neovim::new(session);

    // send some commands
    nvim.command(&command).unwrap();

    let curbuf = nvim.get_current_buf().unwrap();
    debug!("buffer name: {}", curbuf.get_name(&mut nvim).unwrap());

    // we are now subscrided to events related to this buffer
    curbuf.attach(&mut nvim, false, [].to_vec()).unwrap();

    // this is the receiver loop
    loop {
        // read the communication channel for updates
        match receiver.recv() {
            Ok(BufferEvent::Delete) => {
                // buffer is deleted, so let's die!
                break;
            }
            _ => {
                println!("received stuff!");
            }
        }
    }
}

// nnoremap <buffer> <leader>pc :Dispatch cargo build<CR>
// nnoremap <buffer> <leader>pt :Dispatch cargo run<CR>
// nnoremap <buffer> <leader>pf :Dispatch cargo fmt<CR>
