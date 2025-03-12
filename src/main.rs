use std::{
    fs, io::stdout, path::PathBuf, string::String, thread::sleep, time::{Duration, Instant}
};

use clap::Parser;
use crossterm::{cursor, ExecutableCommand, terminal, execute};
use directories::BaseDirs;
use mlua::Lua;

use pet::{Animation, Pet};
use args::Args;

mod pet;
mod args;

fn main() -> Result<(), String> {
    let args = Args::parse();

    let mut stdout = stdout();
    stdout.execute(cursor::Hide).unwrap();

    // Load the pet

    let lua = Lua::new();

    let pet_path_buf = get_config_dir()
        .expect("The configuration directory conldn't be created")
        .join("pets")
        .join(args.pet);
    let pet_path = pet_path_buf.as_path();

    let pet = Pet::load(&lua, pet_path)
        .map_err(|e| format!("Loading the pet failed: {e}"))?;

    println!("Loaded pet:");
    println!("Name: {}", pet.metadata.name);
    println!("Description: {}", pet.metadata.description);
    sleep(Duration::from_secs(1));
    execute!(stdout, terminal::Clear(terminal::ClearType::All)).unwrap();

    // init loop
    let current_state = &pet.metadata.default_state;
    let mut current_anim = pet.states.get(current_state).unwrap().metadata.animation.clone();

    let mut current_frame = 0;

    let mut now = Instant::now();

    let mut last_render = now;
    let mut last_update = now;

    let delay = Duration::from_millis(pet.metadata.global_tick_delay);

    let current_anim_closure = current_anim.clone();
    // Init lua globals
    lua.globals().set(
        "get_current_anim",
        lua.create_function(
            move |_, ()| Ok(current_anim_closure.clone())
        ).unwrap()
    ).unwrap();

    let current_anim_ptr = &mut current_anim as *mut String;
    let current_frame_ptr = &mut current_frame as *mut usize;

    lua.globals().set(
        "set_current_anim",
       lua.create_function_mut(move |_, anim_name: String| {
            unsafe {
                *current_anim_ptr = anim_name;
                *current_frame_ptr = 0;
            }
            Ok(())
        }).unwrap()
    ).unwrap();

    if let Some(f) = &pet.states.get(current_state).unwrap().event_handlers.init {
        f.call::<(), ()>(())
            .map_err(|e| format!("The pet's init function failed: '{}'", e))?;
    }

    loop {
        now = Instant::now();
        let state = pet.states.get(current_state).unwrap();

        if now.duration_since(last_render).as_millis() >= pet.animations.get(current_state).unwrap().metadata.delay.into() {
            let anim = pet.animations.get(&current_anim).unwrap();

            execute!(stdout, terminal::Clear(terminal::ClearType::All)).unwrap();
            println!("{}", anim.frames[current_frame]);

            if current_frame == anim.frames.len() - 1 && anim.name != state.metadata.animation {
                current_anim = state.metadata.animation.clone();
                current_frame = 0;
            }

            current_frame = next_frame(&current_frame, anim);
            last_render = now;
        }

        if state.event_handlers.update.is_some() && now.duration_since(last_update).as_millis() >= state.metadata.update_delay.into() {
            if let Some(f) = &state.event_handlers.update {
                f.call::<(), ()>(())
                    .map_err(|e|
                        format!("The pet's update function failed: '{e}'"))?;
            }

            last_update = now;
        }

        sleep(delay);
    }
}

fn next_frame(frame: &usize, animation: &Animation) -> usize {
    if *frame < animation.frames.len() - 1 {
        frame + 1
    } else {
        0
    }
}

fn get_config_dir() -> Result<PathBuf, String> {
    if let Some(base_dirs) = BaseDirs::new() {
        let path = base_dirs.config_dir().join("a_duk");
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        }

        Ok(path)
    } else {
        Err("BaseDirs couldn't be instantiated".to_string())
    }
}

